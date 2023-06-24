//! Contains the `/yoink` endpoint filter.

use crate::backbone::GetReaderError;
use crate::AppState;
use axum::body::HttpBody;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use hyper::StatusCode;
use shortguid::ShortGuid;
use tracing::info;

pub trait YoinkRoutes {
    /// Provides an API for storing files.
    ///
    /// ```http
    /// GET /yoink/KmC6e8laTnK3dioUSMpM0Q HTTP/1.1
    /// Content-Length: 1024
    /// Content-Type: application/my-type
    ///
    /// your-data
    /// ```
    fn map_yoink_endpoint(self) -> Self;
}

impl<B> YoinkRoutes for Router<AppState, B>
where
    B: HttpBody + Send + Sync + 'static,
    axum::body::Bytes: From<<B as HttpBody>::Data>,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
{
    fn map_yoink_endpoint(self) -> Self {
        self.route("/yoink/:id", get(do_yoink))
    }
}

#[axum::debug_handler]
async fn do_yoink(
    Path(id): Path<ShortGuid>,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    let file = match state.backbone.get_file(id).await {
        Ok(file) => file,
        Err(e) => return Ok(e.into()),
    };

    info!("A yoink was attempted for ID {id}");

    todo!()
}

impl From<GetReaderError> for Response {
    fn from(value: GetReaderError) -> Self {
        match value {
            GetReaderError::UnknownFile(id) => problemdetails::new(StatusCode::NOT_FOUND)
                .with_title("File not found")
                .with_detail(format!("The file with ID {id} could not be found"))
                .with_instance(format!("/yoink/{id}"))
                .with_value("id", id.to_string())
                .into_response(),
            GetReaderError::FileExpired(id) => problemdetails::new(StatusCode::GONE)
                .with_title("File not found")
                .with_detail(format!("The file with ID {id} has expired"))
                .with_instance(format!("/yoink/{id}"))
                .with_value("id", id.to_string())
                .into_response(),
            GetReaderError::FileError(id, e) => {
                problemdetails::new(StatusCode::INTERNAL_SERVER_ERROR)
                    .with_title("File not found")
                    .with_detail(format!("Unable to process file: {e}"))
                    .with_instance(format!("/yoink/{id}"))
                    .with_value("id", id.to_string())
                    .with_value("error", e.to_string())
                    .into_response()
            }
        }
    }
}
