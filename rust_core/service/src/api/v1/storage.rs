use axum::extract::{Extension, State};
use contracts::telemetry::StorageSnapshot;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn storage(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<StorageSnapshot> {
    let snapshot = state.host_telemetry.read().await.storage.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
