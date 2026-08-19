use axum::extract::{Extension, State};
use contracts::{
    default_capability_registry, Capability, CapabilityFamily, HealthResponse, SelfMetricValue,
};

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn health(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<HealthResponse> {
    let uptime_seconds = state.started_at.elapsed().as_secs();
    let snapshot = state.self_metrics.read().await.clone();

    let mut capabilities = default_capability_registry();
    capabilities.insert(
        CapabilityFamily::SelfMetrics,
        match &snapshot.rss_bytes {
            SelfMetricValue::Supported { .. } => Capability::Supported,
            SelfMetricValue::Unavailable { reason } => Capability::Unavailable {
                reason: reason.clone(),
            },
        },
    );

    let response = HealthResponse {
        name: "ai-ops-core".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: contracts::API_VERSION.to_string(),
        uptime_seconds,
        platform: state.platform.platform_name().to_string(),
        arch: state.platform.arch().to_string(),
        capabilities,
        self_cpu_percent: snapshot.cpu_percent,
        self_rss_bytes: snapshot.rss_bytes,
    };

    ApiResponse::new(request_id, Ok(response))
}
