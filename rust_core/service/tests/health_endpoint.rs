//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use ai_ops_core::policy::{ActionTypeRegistry, Environment, ProtectedResourceRegistry};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use contracts::{Envelope, SelfMetricValue};
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

async fn get_health(
    self_metrics: Arc<tokio::sync::RwLock<service::self_metrics::SelfMetricsSnapshot>>,
) -> contracts::HealthResponse {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(
        platform,
        self_metrics,
        host_telemetry,
        None,
        None,
        Arc::new(ActionTypeRegistry::new()),
        Arc::new(ProtectedResourceRegistry::new()),
        Environment::Development,
        None,
    );
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

    envelope.data.expect("data is present on success")
}

#[tokio::test]
async fn health_returns_a_valid_envelope() {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let health = get_health(self_metrics::spawn(platform)).await;

    assert_eq!(health.name, "ai-ops-core");
    assert_eq!(health.api_version, contracts::API_VERSION);
}

/// SRS FR-SYS-002 ("the core reports its own resource usage on every
/// health check"): `self_metrics::spawn`'s own first sample is
/// synchronous, so `self_rss_bytes` is already a real, non-placeholder
/// value on the very first request — no wait needed. `self_cpu_percent`,
/// by contrast, needs a second sample (a real delta over elapsed wall
/// time) before it can ever be `Supported`; on this same first request
/// it's deterministically `Unavailable` — also a real, asserted fact,
/// not a gap left unverified.
#[tokio::test]
async fn health_reports_real_rss_immediately_and_cpu_percent_as_unavailable_on_the_first_sample() {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let health = get_health(self_metrics::spawn(platform)).await;

    match health.self_rss_bytes {
        SelfMetricValue::Supported { value } => {
            assert!(value > 0, "a real process has nonzero RSS")
        }
        other => {
            panic!("expected a real Supported RSS on the immediate first sample, got {other:?}")
        }
    }
    match health.self_cpu_percent {
        SelfMetricValue::Unavailable { reason } => {
            assert_eq!(reason, "insufficient samples yet");
        }
        other => panic!("a lone first sample can never produce a real CPU% delta, got {other:?}"),
    }
}

/// The one code path `health_reports_real_rss_immediately_and_cpu_
/// percent_as_unavailable_on_the_first_sample` can't reach: a genuine
/// `Supported` `self_cpu_percent`, which needs a real second sample.
/// `spawn_with_interval` (unit U62's own test-only override, mirroring
/// unit U53's `PostgresAdapter::with_activity_thresholds` precedent)
/// shrinks the real wait from `self_metrics`'s fixed 5 real seconds down
/// to milliseconds.
#[tokio::test]
async fn health_reports_a_real_supported_cpu_percent_after_a_second_sample() {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn_with_interval(platform, Duration::from_millis(50));
    tokio::time::sleep(Duration::from_millis(200)).await;

    let health = get_health(self_metrics).await;
    match health.self_cpu_percent {
        SelfMetricValue::Supported { value } => {
            assert!(
                value.is_finite() && value >= 0.0,
                "expected a real, finite, non-negative CPU%, got {value}"
            );
        }
        other => panic!("expected a real Supported CPU% after a second sample, got {other:?}"),
    }
}
