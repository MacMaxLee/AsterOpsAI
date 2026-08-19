use axum::extract::{Extension, State};
use contracts::telemetry::ProcessSnapshot;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn processes(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<ProcessSnapshot> {
    let snapshot = state.host_telemetry.read().await.processes.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
