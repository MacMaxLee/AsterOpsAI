//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use contracts::Envelope;
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_a_valid_envelope() {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(platform, self_metrics, host_telemetry, None, None);
    let app = api::router(state);

    let request = Request::builder()
        .uri("/api/v1/health")
        .body(Body::empty())
        .expect("request builds");

    let response = app.oneshot(request).await.expect("router does not fail");
    assert_eq!(response.status(), StatusCode::OK);

    let request_id_header = response
        .headers()
        .get("x-request-id")
        .expect("x-request-id header is set")
        .to_str()
        .expect("header is valid ascii")
        .to_string();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let envelope: Envelope<contracts::HealthResponse> =
        serde_json::from_slice(&body).expect("body is a valid envelope");

    assert!(envelope.success);
    assert!(envelope.error.is_none());
    assert_eq!(envelope.request_id.to_string(), request_id_header);

    let health = envelope.data.expect("data is present on success");
    assert_eq!(health.name, "ai-ops-core");
    assert_eq!(health.api_version, contracts::API_VERSION);
}
