//! Real end-to-end coverage of the tuning plan history view (unit U14):
//! a real axum router (`tower::ServiceExt::oneshot`), a real
//! SQLite-backed `RepositoryHandle`, real rows inserted via
//! `repository::insert_tuning_plan` directly (proving this endpoint's
//! own listing/mapping, not re-proving `core::tuning::start_plan`'s
//! own orchestration, which `core`'s own test suite already covers).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ai_ops_core::policy::{ActionTypeRegistry, ProtectedResourceRegistry};
use ai_ops_core::repository::{self, NewTuningPlan, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
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

async fn open_repo() -> repository::RepositoryHandle {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.sqlite3");
    // Leak the TempDir so the file survives for the rest of the test —
    // integration-test scale, not a real service lifetime concern.
    std::mem::forget(dir);
    repository::init(&RepositoryConfig { db_path }).expect("repository::init")
}

fn target_identity_json() -> String {
    json!({ "kind": "PROCESS", "pid": 999_999, "start_time_ticks": 1 }).to_string()
}

async fn insert_plan(
    repo: &repository::RepositoryHandle,
    created_at: chrono::DateTime<Utc>,
) -> i64 {
    let new = NewTuningPlan {
        created_at,
        target_identity_json: target_identity_json(),
        target_start_time: 1,
        profile: "BALANCED".to_string(),
        mode: "RECOMMEND_ONLY".to_string(),
        status: "COMPLETED".to_string(),
        candidates_json: json!([{
            "action_type": "host.set_process_priority",
            "outcome": "RECOMMENDED",
            "row_id": null,
            "detail": null
        }])
        .to_string(),
    };
    repository::insert_tuning_plan(repo, new)
        .await
        .expect("insert_tuning_plan")
        .id
}

#[tokio::test]
async fn an_empty_repository_has_no_tuning_plans() {
    let repo = open_repo().await;
    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/tuning/plans").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], json!([]));
}

#[tokio::test]
async fn a_plan_appears_with_the_right_fields_and_valid_json_passthrough() {
    let repo = open_repo().await;
    let now = Utc::now();
    let plan_id = insert_plan(&repo, now).await;

    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/tuning/plans").await;
    assert_eq!(status, StatusCode::OK);
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["id"], plan_id);
    assert_eq!(item["profile"], "BALANCED");
    assert_eq!(item["mode"], "RECOMMEND_ONLY");
    assert_eq!(item["status"], "COMPLETED");

    // Both raw-passthrough fields must round-trip as genuinely valid,
    // parseable JSON strings, not opaque text.
    let target: Value =
        serde_json::from_str(item["target_identity_json"].as_str().expect("string field"))
            .expect("target_identity_json is valid JSON");
    assert_eq!(target["pid"], 999_999);
    let candidates: Value =
        serde_json::from_str(item["candidates_json"].as_str().expect("string field"))
            .expect("candidates_json is valid JSON");
    assert_eq!(candidates[0]["action_type"], "host.set_process_priority");
}

#[tokio::test]
async fn more_than_the_limit_returns_exactly_the_limit_most_recent_first() {
    let repo = open_repo().await;
    let now = Utc::now();

    // 105 plans, strictly increasing created_at, oldest first — the
    // response must contain exactly 100, and the very first one must be
    // the most recently created (index 104, created last).
    let mut ids = Vec::new();
    for i in 0..105 {
        let id = insert_plan(&repo, now + Duration::seconds(i)).await;
        ids.push(id);
    }
    let newest_id = *ids.last().expect("at least one plan");

    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/tuning/plans").await;
    assert_eq!(status, StatusCode::OK);
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(
        items.len(),
        100,
        "must be capped at the real limit, not unbounded"
    );
    assert_eq!(
        items[0]["id"], newest_id,
        "must be ordered most-recent-first"
    );
}
