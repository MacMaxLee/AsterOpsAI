use axum::extract::{Extension, State};
use contracts::telemetry::SystemStatusResponse;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn system_status(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<SystemStatusResponse> {
    let snapshot = state.host_telemetry.read().await.system_status.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
