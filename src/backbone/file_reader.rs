use axum::headers::ContentType;
use shared_files::{FileSize, SharedTemporaryFileReader};
use std::borrow::Cow;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::time::Instant;

/// A read accessor for a temporary file.
pub struct FileReader {
    /// The file reader.
    inner: SharedTemporaryFileReader,
    content_type: Option<String>,
    created: Instant,
    expiration_duration: Duration,
}

impl FileReader {
    pub fn new(
        reader: SharedTemporaryFileReader,
        content_type: Option<ContentType>,
        created: Instant,
        expiration_duration: Duration,
    ) -> Self {
        Self {
            inner: reader,
            content_type: content_type.map(|c| c.to_string()),
            created,
            expiration_duration,
        }
    }

    pub fn expiration_date(&self) -> Instant {
        self.created + self.expiration_duration
    }

    pub fn file_size(&self) -> FileSize {
        self.inner.file_size()
    }

    pub fn file_age(&self) -> Duration {
        Instant::now() - self.created
    }

    pub fn content_type(&self) -> Option<Cow<str>> {
        match &self.content_type {
            None => None,
            Some(content_type) => Some(Cow::from(content_type.as_str())),
        }
    }
}

impl AsyncRead for FileReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(read) => {
                // TODO: Increment metrics for reading from the file
                Poll::Ready(read)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
