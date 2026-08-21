//! Real end-to-end coverage of the generic action-propose endpoint (unit
//! U26): a real axum router, a real SQLite-backed `RepositoryHandle`,
//! and a real spawned process as the target, proving `security.
//! suspend_process` — fully built and core-tested since unit U11 but
//! never previously reachable over HTTP — genuinely proposes, appears
//! in the real Policy inbox, gets granted through unit U22's existing
//! endpoint, and really stops the target process. Unlike unit U25's own
//! capstone, this one needs no `Environment::Production` override:
//! `RiskLevel::High` requires approval in every environment.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use ai_ops_core::policy::{Environment, ProtectedResourceRegistry};
use ai_ops_core::repository::{self, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use service::{actions::build_action_registry, api, self_metrics, state::AppState, telemetry};
use support::SpawnedTarget;
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
        Arc::new(build_action_registry()),
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

async fn post_json(app: axum::Router, path: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds");
    let response = app.oneshot(request).await.expect("router does not fail");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let json: Value = serde_json::from_slice(&bytes).expect("body is valid json");
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

/// The capstone: propose a real `security.suspend_process` action ->
/// real `PENDING_APPROVAL` row (reachable under the *default*
/// `Development` environment, since `RiskLevel::High` always requires
/// approval) -> appears in the real Policy inbox -> granted through
/// unit U22's existing endpoint -> the target process is really
/// stopped.
#[tokio::test]
async fn proposing_suspend_process_reaches_the_inbox_and_grant_really_stops_the_target() {
    let repo = open_repo().await;
    let mut target = SpawnedTarget::spawn();

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        "/api/v1/actions/propose",
        json!({
            "action_type": "security.suspend_process",
            "pid": target.pid,
            "start_time_ticks": target.start_time_ticks,
            "resource_name": "sleep-test-target",
            "requested_by": "tester",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["status"], "PENDING_APPROVAL");
    assert!(!body["data"]["approval_expires_at"].is_null());
    let row_id = body["data"]["row_id"]
        .as_i64()
        .expect("row_id is an integer");

    let app = build_app(Some(repo.clone())).await;
    let (_, body) = get_json(app, "/api/v1/policy/pending").await;
    let pending_ids: Vec<i64> = body["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|item| item["id"].as_i64().expect("id is an integer"))
        .collect();
    assert!(
        pending_ids.contains(&row_id),
        "the proposed suspend action must be visible in the real inbox: {pending_ids:?}"
    );

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "grant body: {body:?}");

    // Real proof: the target process is genuinely stopped.
    let platform = platform::current_platform_adapter();
    let stopped = platform
        .is_process_stopped(target.pid as u32)
        .expect("is_process_stopped");
    assert!(
        stopped,
        "the real process must now be stopped, proving the full \
         propose -> inbox -> grant -> execute loop ran for real"
    );

    // Real, safe teardown: SIGKILL terminates a stopped process too.
    target.kill_and_reap();
}

/// Unit U29's own capstone: the real, correctness-critical claim that a
/// resumable action must genuinely stop appearing once it's actually
/// been resumed — `rollback()` never mutates the original row, so this
/// proves `find_resumable_action`'s `NOT IN (... ROLLED_BACK)` exclusion
/// for real, not from reading the SQL alone.
#[tokio::test]
async fn a_resumable_action_appears_after_grant_and_disappears_after_rollback() {
    let repo = open_repo().await;
    let mut target = SpawnedTarget::spawn();
    let resumable_path = format!(
        "/api/v1/actions/resumable?pid={}&start_time_ticks={}",
        target.pid, target.start_time_ticks
    );

    let app = build_app(Some(repo.clone())).await;
    let (_, body) = get_json(app, &resumable_path).await;
    assert_eq!(
        body["data"],
        json!([]),
        "nothing has been executed against this target yet"
    );

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        "/api/v1/actions/propose",
        json!({
            "action_type": "security.suspend_process",
            "pid": target.pid,
            "start_time_ticks": target.start_time_ticks,
            "resource_name": "sleep-test-target",
            "requested_by": "tester",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "propose body: {body:?}");
    let row_id = body["data"]["row_id"]
        .as_i64()
        .expect("row_id is an integer");

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "grant body: {body:?}");

    let app = build_app(Some(repo.clone())).await;
    let (_, body) = get_json(app, &resumable_path).await;
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1, "the granted suspend must now be resumable");
    assert_eq!(items[0]["row_id"], row_id);
    assert_eq!(items[0]["action_type"], "security.suspend_process");
    assert!(!items[0]["executed_at"].is_null());

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/rollback"),
        json!({ "rolled_back_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "rollback body: {body:?}");

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, &resumable_path).await;
    assert_eq!(
        body["data"],
        json!([]),
        "a successfully rolled-back action must stop appearing as resumable"
    );

    target.kill_and_reap();
}

#[tokio::test]
async fn proposing_an_unknown_action_type_is_a_real_unsupported() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        "/api/v1/actions/propose",
        json!({
            "action_type": "not.a.real.action.type",
            "pid": target.pid,
            "start_time_ticks": target.start_time_ticks,
            "resource_name": "sleep-test-target",
            "requested_by": "tester",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "body: {body:?}");
    assert_eq!(body["error"]["code"], "UNSUPPORTED");
}

#[tokio::test]
async fn proposing_malformed_parameters_is_a_real_bad_request_not_an_internal_error() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        "/api/v1/actions/propose",
        json!({
            "action_type": "host.set_process_cpu_affinity",
            "pid": target.pid,
            "start_time_ticks": target.start_time_ticks,
            "resource_name": "sleep-test-target",
            // Missing the required "cpus" key entirely.
            "parameters": {},
            "requested_by": "tester",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body:?}");
    assert_eq!(body["error"]["code"], "BAD_REQUEST");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("message is a string")
            .contains("cpus"),
        "expected the real validator's own message naming the missing field, got {body:?}"
    );
}

/// Unit U43 (ADR 0034's own named "double-suspend" gap, closed;
/// ADR 0048): proposing `security.suspend_process` twice against the
/// same real spawned target — the first genuinely reaches
/// `PENDING_APPROVAL`, the second is a real `400`, not a second
/// `PENDING_APPROVAL` row silently coexisting with the first.
#[tokio::test]
async fn proposing_the_same_suspend_twice_against_the_same_target_is_a_real_bad_request() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let body = json!({
        "action_type": "security.suspend_process",
        "pid": target.pid,
        "start_time_ticks": target.start_time_ticks,
        "resource_name": "sleep-test-target",
        "requested_by": "tester",
    });

    let app = build_app(Some(repo.clone())).await;
    let (first_status, first_body) = post_json(app, "/api/v1/actions/propose", body.clone()).await;
    assert_eq!(first_status, StatusCode::OK, "first body: {first_body:?}");
    assert_eq!(first_body["data"]["status"], "PENDING_APPROVAL");

    let app = build_app(Some(repo)).await;
    let (second_status, second_body) = post_json(app, "/api/v1/actions/propose", body).await;
    assert_eq!(
        second_status,
        StatusCode::BAD_REQUEST,
        "second body: {second_body:?}"
    );
    assert_eq!(second_body["error"]["code"], "BAD_REQUEST");
}
