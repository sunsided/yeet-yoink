use app_config::AppConfig;
use backend_traits::{
    Backend, BackendCommand, BackendCommandSender, BackendRegistration, RegisterBackendError,
    TryCreateFromConfig,
};
use file_distribution::FileProvider;
use rendezvous::RendezvousGuard;
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::{JoinError, JoinHandle};
use tracing::{debug, error, info, warn};

const EVENT_BUFFER_SIZE: usize = 64;

pub struct BackendRegistry {
    handle: JoinHandle<()>,
    sender: Cell<Option<Sender<BackendCommand>>>,
}

impl BackendRegistry {
    pub fn builder(
        cleanup_rendezvous: RendezvousGuard,
        file_accessor: FileProvider,
    ) -> BackendRegistryBuilder {
        BackendRegistryBuilder::new(cleanup_rendezvous, file_accessor)
    }

    /// Creates a new instance of the [`BackendRegistry`].
    ///
    /// # Arguments
    ///
    /// - `cleanup_rendezvous`: A `RendezvousGuard` used for cleanup.
    /// - `backends`: A `Vec<Backend>` containing the list of backends.
    /// - `file_accessor`: A `FileProvider` used for file access.
    ///
    /// # Returns
    ///
    /// A new instance of [`BackendRegistry`].
    fn new(
        cleanup_rendezvous: RendezvousGuard,
        backends: Vec<Backend>,
        file_accessor: FileProvider,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(EVENT_BUFFER_SIZE);
        let handle = tokio::spawn(Self::handle_events(
            backends,
            receiver,
            cleanup_rendezvous,
            file_accessor,
        ));
        Self {
            handle,
            sender: Cell::new(Some(sender)),
        }
    }

    /// Retrieves the sender of the backend command.
    ///
    /// # Returns
    ///
    /// - `Some(sender)`: If the sender exists, it returns the backend command sender.
    /// - `None`: If the sender is not available, it returns `None`.
    pub(crate) fn get_sender(&self) -> Option<BackendCommandSender> {
        self.sender.take().map(BackendCommandSender::from)
    }

    pub async fn join(self) -> Result<(), JoinError> {
        self.handle.await
    }

    /// Handles backend-related events asynchronously.
    ///
    /// This function continuously receives events from a receiver and performs appropriate tasks
    /// based on the received event type.
    ///
    /// # Arguments
    /// - `backends`: A vector of `Backend` objects to be used for processing.
    /// - `receiver`: Receiver end of a channel where backend commands are sent.
    /// - `cleanup_rendezvous`: A `RendezvousGuard` instance used to signal when all backend tasks
    ///   have finished for proper cleanup.
    /// - `file_accessor`: A `FileProvider` to provide access to the files to be distributed.
    ///
    /// # Behavior
    /// This function works in a loop, where it awaits for a `BackendCommand` from `receiver`.
    /// Based on the command, it performs different tasks. If the command is
    /// `BackendCommand::DistributeFile`, it distributes the file across the backends.
    ///
    /// # Error Handling
    /// If an error occurs during the distribution of a file on a backend, it logs a warning message
    /// but continues to next backend.
    async fn handle_events(
        backends: Vec<Backend>,
        mut receiver: Receiver<BackendCommand>,
        cleanup_rendezvous: RendezvousGuard,
        file_accessor: FileProvider,
    ) {
        let backends = Arc::new(backends);
        let file_accessor = Arc::new(file_accessor);
        while let Some(event) = receiver.recv().await {
            let task_guard = cleanup_rendezvous.fork();
            let backends = backends.clone();
            let file_accessor = file_accessor.clone();

            // Spawn the task onto the executor to avoid race conditions.
            // We do this such that uploads do not block downloads, and vice versa.
            tokio::task::spawn(async move {
                match event {
                    BackendCommand::DistributeFile(id, summary) => {
                        debug!(file_id = %id, "Handling distribution of file {id}", id = id);

                        // TODO: #55 Spawn distribution tasks in background

                        // TODO: #57 Initiate tasks in priority order?
                        for backend in backends.iter() {
                            match backend
                                .distribute_file(id, summary.clone(), file_accessor.clone())
                                .await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    warn!(file_id = %id, "Failed to distribute file using backend {tag}: {error}", tag = backend.tag(), error = e);
                                }
                            }
                        }
                    }
                    BackendCommand::ReceiveFile(id, sender) => {
                        debug!(file_id = %id, "Handling download of file {id}", id = id);
                        todo!("Implement download of file")
                    }
                }

                debug!("Closing background event handling");
                task_guard.completed();
            });
        }

        // TODO: Wait until all currently running tasks have finished.
        debug!("Closing backend event loop");
        cleanup_rendezvous.completed();
    }
}

pub struct BackendRegistryBuilder {
    backends: Vec<Backend>,
    cleanup_rendezvous: RendezvousGuard,
    file_accessor: FileProvider,
}

impl BackendRegistration for BackendRegistryBuilder {
    fn add_backends<T>(self, config: &AppConfig) -> Result<(), RegisterBackendError>
    where
        T: TryCreateFromConfig,
    {
        self.add_backends::<T>(config)?;
        Ok(())
    }
}

impl BackendRegistryBuilder {
    fn new(cleanup_rendezvous: RendezvousGuard, file_accessor: FileProvider) -> Self {
        Self {
            backends: Vec::default(),
            cleanup_rendezvous,
            file_accessor,
        }
    }

    pub fn build(self) -> BackendRegistry {
        BackendRegistry::new(self.cleanup_rendezvous, self.backends, self.file_accessor)
    }

    /// Adds backends to the application.
    ///
    /// This function takes a type `T` that implements the `TryCreateFromConfig` trait, and a reference to an `AppConfig`.
    /// It tries to create backends from the given configuration using the `try_from_config` method of `T`.
    /// If successful, it adds the created backends to the application using the `add_backends_from_iter` method.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to an `AppConfig` that provides the configuration for creating the backends.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the backends were added successfully, otherwise returns a `RegisterBackendError`.
    ///
    /// # Errors
    ///
    /// This function may return a `RegisterBackendError` if an error occurs during the registration of the backends.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::backend::Backend;
    ///
    /// let mut app = App::new();
    /// let config = AppConfig::new();
    ///
    /// match app.add_backends::<MyBackend>(&config) {
    ///     Ok(()) => println!("Backends added successfully"),
    ///     Err(error) => eprintln!("Failed to add backends: {}", error),
    /// };
    /// ```
    pub fn add_backends<T>(
        self,
        config: &AppConfig,
    ) -> Result<BackendRegistryBuilder, RegisterBackendError>
    where
        T: TryCreateFromConfig,
    {
        match T::try_from_config(config)
            .map_err(|e| RegisterBackendError::TryCreateFromConfig(Box::new(e)))
        {
            Ok(backends) => {
                if !backends.is_empty() {
                    info!(
                "Registering {count} {backend} backend{plural} (backend version {backend_version})",
                count = backends.len(),
                backend = T::backend_name(),
                backend_version = T::backend_version(),
                plural = if backends.len() == 1 { "" } else { "s" }
            );
                    Ok(self.add_backends_from_iter(backends))
                } else {
                    Ok(self)
                }
            }
            Err(e) => {
                error!("Failed to initialize Memcached backends: {}", e);
                Err(e)
            }
        }
    }

    /// Registers multiple backends.
    fn add_backends_from_iter<I: IntoIterator<Item = Backend>>(
        mut self,
        backends: I,
    ) -> BackendRegistryBuilder {
        self.backends.extend(backends);
        self
    }
}
