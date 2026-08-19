use axum::extract::{Extension, State};
use contracts::telemetry::DeviceSnapshot;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn devices(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<DeviceSnapshot> {
    let snapshot = state.host_telemetry.read().await.devices.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
