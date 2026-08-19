use axum::extract::{Extension, State};
use contracts::telemetry::NetworkSnapshot;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn network(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<NetworkSnapshot> {
    let snapshot = state.host_telemetry.read().await.network.clone();
    ApiResponse::new(request_id, Ok(snapshot))
}
