use axum::extract::{Extension, State};
use contracts::telemetry::MemorySnapshot;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn memory(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<MemorySnapshot> {
    let snapshot = state.host_telemetry.read().await.memory.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
