use crate::backbone::backbone::BackboneCommand;
use crate::backbone::file_writer_guard::WriteResult;
use shared_files::SharedTemporaryFile;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::Receiver;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};
use uuid::Uuid;

/// The duration for which to keep each file alive.
const TEMPORAL_LEASE: Duration = Duration::from_secs(5 * 60);

#[derive(Debug)]
pub(crate) struct FileRecord;

#[derive(Debug)]
struct Inner {
    file: Option<SharedTemporaryFile>,
}

impl FileRecord {
    pub fn new(
        id: Uuid,
        file: SharedTemporaryFile,
        backbone_command: Sender<BackboneCommand>,
        writer_command: Receiver<WriteResult>,
    ) -> Self {
        let inner = Arc::new(RwLock::new(Inner { file: Some(file) }));
        let _ = tokio::spawn(Self::lifetime_handler(
            id,
            inner.clone(),
            backbone_command,
            writer_command,
        ));
        Self {}
    }

    /// Controls the lifetime of the entry in the backbone.
    ///
    /// This method will:
    ///
    /// - Wait until the file is buffered to disk completely,
    /// - Apply a temporal lease to the file (keeping it alive for a certain time).
    /// - Remove the file from the registry after the time is over.
    async fn lifetime_handler(
        id: Uuid,
        mut inner: Arc<RwLock<Inner>>,
        backbone_command: mpsc::Sender<BackboneCommand>,
        writer_command: Receiver<WriteResult>,
    ) {
        // Before starting the timeout, wait for the write to the file to complete.
        match writer_command.await {
            Ok(WriteResult::Success(hashes)) => {
                info!("File writing completed: {}", hashes);
            }
            Ok(WriteResult::Failed) => {
                warn!("Writing to the file failed");
                Self::close_file(&mut inner).await;
                Self::remove_writer(id, backbone_command).await;
                return;
            }
            Err(e) => {
                warn!("The file writer channel failed: {e}");
                Self::close_file(&mut inner).await;
                Self::remove_writer(id, backbone_command).await;
                return;
            }
        }

        // Indicate the file is ready for processing.
        if let Err(error) = backbone_command
            .send(BackboneCommand::ReadyForDistribution(id))
            .await
        {
            warn!("The backbone writer channel was closed while indicating a termination for file with ID {id}: {error}");
            return;
        }

        // Keep the file open for readers.
        Self::apply_temporal_lease(&id, TEMPORAL_LEASE).await;
        info!("Read lease timed out for file {id}; removing it");

        // Gracefully close the file.
        Self::remove_writer(id, backbone_command).await;
    }

    async fn apply_temporal_lease(id: &Uuid, duration: Duration) {
        info!("File {id} will accept new readers for {duration:?}");
        tokio::time::sleep(duration).await
    }

    async fn close_file(inner: &mut Arc<RwLock<Inner>>) {
        let mut inner = inner.write().await;
        inner.file.take();
    }

    async fn remove_writer(id: Uuid, backbone_command: Sender<BackboneCommand>) {
        if let Err(error) = backbone_command
            .send(BackboneCommand::RemoveWriter(id))
            .await
        {
            warn!("The backbone writer channel was closed while indicating a termination for file with ID {id}: {error}");
        }
    }
}