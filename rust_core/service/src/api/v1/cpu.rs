use axum::extract::{Extension, State};
use contracts::telemetry::CpuSnapshot;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn cpu(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<CpuSnapshot> {
    let snapshot = state.host_telemetry.read().await.cpu.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
