use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use contracts::{ApiError, Envelope};
use uuid::Uuid;

/// What a handler computes before it's wrapped into the wire envelope.
pub type ApiResult<T> = Result<T, ApiError>;

/// Wraps a handler's `ApiResult<T>` into the versioned `Envelope`, picking an
/// HTTP status from the closed `ApiError` enum on failure.
pub struct ApiResponse<T> {
    request_id: Uuid,
    result: ApiResult<T>,
}

impl<T> ApiResponse<T> {
    pub fn new(request_id: Uuid, result: ApiResult<T>) -> Self {
        Self { request_id, result }
    }
}

impl<T: serde::Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        match self.result {
            Ok(data) => {
                let envelope = Envelope::ok(self.request_id, data);
                (StatusCode::OK, Json(envelope)).into_response()
            }
            Err(error) => {
                let status = status_for(&error);
                let envelope: Envelope<T> = Envelope::err(self.request_id, error);
                (status, Json(envelope)).into_response()
            }
        }
    }
}

fn status_for(error: &ApiError) -> StatusCode {
    match error {
        ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
        ApiError::NotFound => StatusCode::NOT_FOUND,
        ApiError::Unsupported(_) => StatusCode::NOT_IMPLEMENTED,
        ApiError::PermissionRequired(_) => StatusCode::FORBIDDEN,
        ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
