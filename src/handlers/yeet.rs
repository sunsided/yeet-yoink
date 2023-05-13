//! Contains the `/yeet` endpoint filter.

use crate::AppState;
use async_tempfile::TempFile;
use axum::body::HttpBody;
use axum::extract::BodyStream;
use axum::response::{IntoResponse, Response};
use axum::routing::{post, MethodRouter};
use hyper::body::Buf;
use hyper::StatusCode;
use sha2::Digest;
use std::convert::Infallible;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tracing::debug;

const ROUTE: &'static str = "yeet";

/// Provides metrics.
///
/// ```http
/// GET /metrics
/// ```
pub fn yeet_endpoint<S>() -> MethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    post(do_yeet)
}

async fn do_yeet(mut stream: BodyStream) -> Result<Response, Infallible> {
    // info!("{:?}", headers);

    // TODO: https://docs.rs/axum/latest/axum/struct.TypedHeader.html

    // TODO: let content_length = headers.get("Content-Length");
    // TODO: let content_type = headers.get("Content-Type");

    // Add server-side validation if header is present.
    // TODO: let content_md5 = headers.get("Content-MD5");

    // TODO: Allow capacity?
    let mut file = match TempFile::new().await {
        Ok(file) => file,
        Err(e) => {
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create temporary file: {e}"),
            )
                .into_response())
        }
    };

    debug!(
        "Buffering request payload to {file:?}",
        file = file.file_path()
    );

    // if let Some(n) = content_length {
    //    debug!("Expecting {value:?} bytes", value = n);
    // }

    let mut stream = Box::pin(stream);
    let mut md5 = md5::Context::new();
    let mut sha256 = sha2::Sha256::new();

    let mut bytes_written = 0;
    while let Some(result) = stream.next().await {
        let mut data = match result {
            Ok(data) => data,
            Err(e) => {
                return Ok((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to obtain data from the read stream: {e}"),
                )
                    .into_response())
            }
        };

        while data.has_remaining() {
            let chunk = data.chunk();
            md5.consume(chunk);
            sha256.update(chunk);

            match file.write(&chunk).await {
                Ok(0) => {}
                Ok(n) => {
                    bytes_written += n;
                    data.advance(n);
                }
                Err(e) => {
                    return Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to write to temporary file: {e}"),
                    )
                        .into_response())
                }
            }
        }

        match file.sync_data().await {
            Ok(_) => {}
            Err(e) => {
                return Ok((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to flush data to temporary file: {e}"),
                )
                    .into_response())
            }
        }

        // TODO: Wake up consumers
    }

    let md5 = md5.compute();
    let sha256 = sha256.finalize();

    debug!(
        "Stream ended, buffered {bytes} bytes to disk; MD5 {md5:x}, SHA256 {sha256:x}",
        bytes = bytes_written,
        md5 = md5,
        sha256 = sha256
    );

    Ok("".into_response())
}
