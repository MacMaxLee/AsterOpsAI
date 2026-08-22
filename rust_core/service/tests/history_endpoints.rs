//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ai_ops_core::policy::{ActionTypeRegistry, Environment, ProtectedResourceRegistry};
use ai_ops_core::repository::{self, RepositoryConfig, TelemetrySnapshotRow, WriteCommand};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use serde_json::Value;
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

async fn build_app(repository: Option<repository::RepositoryHandle>) -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), repository.clone());
    let state = AppState::new(
        platform,
        self_metrics,
        host_telemetry,
        repository,
        None,
        Arc::new(ActionTypeRegistry::new()),
        Arc::new(ProtectedResourceRegistry::new()),
        Environment::Development,
        None,
    );
    api::router(state)
}

async fn get_json(app: axum::Router, path: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request builds");
    let response = app.oneshot(request).await.expect("router does not fail");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let json: Value = serde_json::from_slice(&body).expect("body is valid json");
    (status, json)
}

#[tokio::test]
async fn history_is_unavailable_without_a_repository() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/history/cpu?range=last_hour").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

/// SRS FR-HIST-001: history is queryable over the range tiers this
/// endpoint exposes (last_hour exercised here; 24h/7d/30d/custom share
/// the same `resolve_range` code path).
#[tokio::test]
async fn history_returns_persisted_points_once_a_repository_is_wired_up() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.sqlite3");
    let repo = repository::init(&RepositoryConfig { db_path }).unwrap();

    // Insert a real row directly (not waiting on the sampler's real 1s/10s
    // cadence) so this test is fast and deterministic.
    let row = TelemetrySnapshotRow {
        ts: Utc::now(),
        cpu_aggregate_util_pct: Some(42.0),
        cpu_aggregate_util_state: "SUPPORTED".to_string(),
        cpu_load_avg_1m: Some(0.5),
        cpu_pressure: "NORMAL".to_string(),
        cpu_per_core_json: None,
        mem_total_bytes: Some(4_000_000_000),
        mem_used_bytes: Some(1_000_000_000),
        mem_used_bytes_state: "SUPPORTED".to_string(),
        mem_available_bytes: Some(3_000_000_000),
        mem_swap_used_bytes: Some(0),
        mem_pressure: "NORMAL".to_string(),
        storage_read_bytes_ps: Some(0.0),
        storage_write_bytes_ps: Some(0.0),
        storage_volumes_json: None,
        net_rx_bytes_ps: Some(0.0),
        net_tx_bytes_ps: Some(0.0),
        net_interfaces_json: None,
        process_total_count: Some(10),
        device_count: Some(2),
        containerized: false,
    };
    repo.command_tx
        .send(WriteCommand::InsertTelemetrySnapshot(Box::new(row)))
        .await
        .unwrap();

    // build_app's own sampler also persists an immediate first-tick row
    // (using real host data) once spawned below, alongside the one we just
    // inserted by hand — so this asserts "our row is present," not "our row
    // is the only one."
    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/history/cpu?range=last_hour").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    let points = body["data"]["points"].as_array().unwrap();
    assert!(!points.is_empty());
    assert!(
        points
            .iter()
            .any(|p| p["aggregate_utilization_percent"]["avg"].as_f64() == Some(42.0)),
        "expected a point with avg=42.0 among {points:?}"
    );
}

#[tokio::test]
async fn custom_range_without_from_is_a_bad_request() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/history/cpu?range=custom").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "BAD_REQUEST");
}

/// Regression test: `#[serde(rename_all = "snake_case")]` alone renders
/// `Last24h`/`Last7d`/`Last30d` as `last24h`/`last7d`/`last30d` (no
/// underscore before a digit), not the `last_24h`/`last_7d`/`last_30d` this
/// API's own query contract specifies. A live E2E run against the real
/// service caught this — `?range=last_30d` returned a serde deserialize
/// error, not the documented tier resolution.
#[tokio::test]
async fn digit_containing_range_params_use_underscored_names() {
    for range in ["last_24h", "last_7d", "last_30d"] {
        let app = build_app(None).await;
        let (status, body) = get_json(app, &format!("/api/v1/history/cpu?range={range}")).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "range={range} should parse (reaching the repository check, not a query-deserialize error); got body {body:?}"
        );
        assert_eq!(body["error"]["code"], "UNAVAILABLE");
    }
}
