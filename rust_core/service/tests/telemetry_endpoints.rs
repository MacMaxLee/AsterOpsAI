//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

async fn build_app() -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(platform, self_metrics, host_telemetry, None);
    api::router(state)
}

async fn get_envelope(app: axum::Router, path: &str) -> Value {
    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request builds");
    let response = app.oneshot(request).await.expect("router does not fail");
    assert_eq!(response.status(), StatusCode::OK, "path: {path}");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let envelope: Value = serde_json::from_slice(&body).expect("body is valid json");
    assert_eq!(envelope["success"], true, "path: {path}");
    assert!(envelope["error"].is_null(), "path: {path}");
    envelope
}

macro_rules! endpoint_smoke_test {
    ($name:ident, $path:literal) => {
        #[tokio::test]
        async fn $name() {
            let app = build_app().await;
            let envelope = get_envelope(app, $path).await;
            assert!(envelope["data"].is_object(), "path: {}", $path);
        }
    };
}

endpoint_smoke_test!(cpu_returns_a_valid_envelope, "/api/v1/cpu");
endpoint_smoke_test!(memory_returns_a_valid_envelope, "/api/v1/memory");
endpoint_smoke_test!(storage_returns_a_valid_envelope, "/api/v1/storage");
endpoint_smoke_test!(network_returns_a_valid_envelope, "/api/v1/network");
endpoint_smoke_test!(processes_returns_a_valid_envelope, "/api/v1/processes");
endpoint_smoke_test!(devices_returns_a_valid_envelope, "/api/v1/devices");
endpoint_smoke_test!(
    system_status_returns_a_valid_envelope,
    "/api/v1/system/status"
);
