//! Real end-to-end coverage of the host performance analysis endpoint
//! (unit U19): a real axum router, real `TelemetrySnapshotRow` history
//! inserted directly into a real SQLite-backed `RepositoryHandle`,
//! proving `analysis::classify_host` itself ran against it — not a
//! hand-built fixture response.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

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
    let state = AppState::new(platform, self_metrics, host_telemetry, repository, None);
    api::router(state)
}

/// Real discovery while writing this file: `telemetry::sampler::spawn`
/// synchronously persists a first real snapshot the instant it's handed
/// `Some(repository)` (`sampler.rs`'s `spawn` calls
/// `try_persist_telemetry_snapshot` on its very first tick, before ever
/// returning). That means "a repository that started but has zero
/// snapshots yet" is unreachable through `build_app`'s normal wiring —
/// by the time a request lands, the live sampler has already written at
/// least one row. To exercise the genuinely-empty-history path this
/// helper deliberately decouples the two: the sampler gets `None` (so it
/// never persists), while `AppState.repository` still gets the real,
/// empty repository, so the handler's own query really does see zero
/// rows.
async fn build_app_with_empty_repository(repository: repository::RepositoryHandle) -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(
        platform,
        self_metrics,
        host_telemetry,
        Some(repository),
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

fn snapshot_row(ts: chrono::DateTime<Utc>, cpu_pressure: &str) -> TelemetrySnapshotRow {
    TelemetrySnapshotRow {
        ts,
        cpu_aggregate_util_pct: Some(95.0),
        cpu_aggregate_util_state: "SUPPORTED".to_string(),
        cpu_load_avg_1m: Some(8.0),
        cpu_pressure: cpu_pressure.to_string(),
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
    }
}

#[tokio::test]
async fn host_analysis_is_unavailable_without_a_repository() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/host").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn an_empty_repository_returns_a_real_unknown_verdict_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.sqlite3");
    let repo = repository::init(&RepositoryConfig { db_path }).unwrap();

    let app = build_app_with_empty_repository(repo).await;
    let (status, body) = get_json(app, "/api/v1/analysis/host").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["bottleneck"], "UNKNOWN");
}

#[tokio::test]
async fn sustained_critical_cpu_history_produces_a_real_cpu_bottleneck() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.sqlite3");
    let repo = repository::init(&RepositoryConfig { db_path }).unwrap();

    let now = Utc::now();
    for i in 0..5 {
        let row = snapshot_row(now - chrono::Duration::seconds(60 - i), "CRITICAL");
        repo.command_tx
            .send(WriteCommand::InsertTelemetrySnapshot(Box::new(row)))
            .await
            .unwrap();
    }
    // Let the writer thread drain the channel before reading it back.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/analysis/host").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"]["bottleneck"], "CPU",
        "real sustained CRITICAL cpu_pressure history must classify as a real CPU bottleneck, \
         got {body:?}"
    );
    let domain_signals = body["data"]["domain_signals"].as_array().unwrap();
    let cpu_signal = domain_signals
        .iter()
        .find(|s| s["domain"] == "CPU")
        .expect("a CPU domain signal exists");
    assert_eq!(cpu_signal["tier"], "CRITICAL");
    assert!(cpu_signal["sample_count"].as_u64().unwrap() >= 5);
}
