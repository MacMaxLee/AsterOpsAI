//! Real end-to-end coverage of the policy approval inbox (unit U13), and
//! its grant -> authorize -> execute path (unit U22): a real axum router
//! (`tower::ServiceExt::oneshot` — real requests, real JSON, no network
//! listener needed), a real SQLite-backed `RepositoryHandle`, real rows
//! inserted via `repository::propose_action` directly, and — for the
//! grant-path tests — a real spawned process as the action's target, so
//! granting genuinely runs `core::actions::execute` against something
//! real rather than a fixture.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use ai_ops_core::policy::hash::compute_parameters_hash;
use ai_ops_core::policy::{
    ActionStatus, Environment, ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use ai_ops_core::repository::{self, NewProposedAction, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
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
        // Grant/reject never call `evaluate()` again (confirmed by
        // direct read, unit U25) — this file's own tests never touch
        // `policy_environment`, so `Development` (the same
        // no-behavior-change default `tuning_endpoints.rs` uses) is
        // fine here unconditionally.
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

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

async fn insert_pending_for(
    repo: &repository::RepositoryHandle,
    requested_by: &str,
    action_type: &str,
    target: &SpawnedTarget,
    parameters: Value,
) -> i64 {
    let now = Utc::now();
    let parameters_json = parameters.to_string();
    let parameters_hash = compute_parameters_hash(&parameters).expect("compute_parameters_hash");
    let new = NewProposedAction {
        created_at: now,
        action_type: action_type.to_string(),
        target_identity_json: target.target_identity_json(),
        target_start_time: target.start_time_ticks,
        risk_classification: "MEDIUM".to_string(),
        status: ActionStatus::PendingApproval.as_str().to_string(),
        evidence_json: "{}".to_string(),
        requested_by: requested_by.to_string(),
        parameters_json,
        parameters_hash,
        resource_descriptor_json: serde_json::to_string(&resource()).expect("serialize resource"),
        approval_expires_at: Some(now + chrono::Duration::seconds(300)),
        rollback_of: None,
    };
    repository::propose_action(repo, new)
        .await
        .expect("propose_action")
        .id
}

#[tokio::test]
async fn an_empty_repository_has_no_pending_actions() {
    let repo = open_repo().await;
    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], json!([]));
}

#[tokio::test]
async fn a_pending_action_appears_in_the_inbox_with_the_right_fields() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "host.set_process_cpu_affinity",
        &target,
        json!({ "cpus": [0] }),
    )
    .await;

    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(status, StatusCode::OK);
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["id"], row_id);
    assert_eq!(item["action_type"], "host.set_process_cpu_affinity");
    assert_eq!(item["risk_classification"], "MEDIUM");
    assert_eq!(item["resource_kind"], "PROCESS");
    assert_eq!(item["resource_name"], "sleep-test-target");
    assert_eq!(item["requested_by"], "tester");
    assert!(!item["approval_expires_at"].is_null());
}

#[tokio::test]
async fn granting_a_reversible_action_really_executes_it() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "host.set_process_cpu_affinity",
        &target,
        json!({ "cpus": [0] }),
    )
    .await;

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");

    // Real proof the action actually ran: the target process's real CPU
    // affinity mask changed, not just the row's own status column.
    let platform = platform::current_platform_adapter();
    let affinity = platform
        .get_process_cpu_affinity(target.pid as u32)
        .expect("get_process_cpu_affinity");
    assert_eq!(
        affinity.cpus,
        std::collections::BTreeSet::from([0]),
        "the real process affinity must now be exactly {{0}}"
    );

    let row = repository::get_action(&repo, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "EXECUTED");
    assert!(row.executed_at.is_some());

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(
        body["data"],
        json!([]),
        "an executed action must leave the inbox"
    );
}

#[tokio::test]
async fn a_grant_whose_target_vanished_before_execution_reports_a_real_failure() {
    let repo = open_repo().await;
    let mut target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "host.set_process_cpu_affinity",
        &target,
        json!({ "cpus": [0] }),
    )
    .await;
    // Kill the real target before granting — execute()'s own re-verify
    // stage must catch this for real, not silently "succeed."
    target.kill_and_reap();

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a real execution failure must not be reported as success: {body:?}"
    );
    assert_eq!(body["success"], false);

    let row = repository::get_action(&repo, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(
        row.status, "FAILED",
        "the row must land on a real terminal failure state, not stay stuck or appear executed"
    );
}

#[tokio::test]
async fn rejecting_a_pending_action_records_a_distinct_audit_event() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "host.set_process_cpu_affinity",
        &target,
        json!({ "cpus": [0] }),
    )
    .await;

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/reject"),
        json!({ "rejected_by": "reviewer" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");

    let row = repository::get_action(&repo, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "DENIED");

    let event_type = repository::latest_audit_event_type(&repo)
        .await
        .expect("latest_audit_event_type")
        .expect("an audit event exists");
    assert_eq!(
        event_type, "policy.rejected",
        "a human rejection must be audited distinctly from an automatic denial"
    );
}

#[tokio::test]
async fn granting_an_already_granted_action_is_a_bad_request_not_a_panic() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "host.set_process_cpu_affinity",
        &target,
        json!({ "cpus": [0] }),
    )
    .await;

    let app = build_app(Some(repo.clone())).await;
    let (status, _) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver-again" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "BAD_REQUEST");
}

/// Unit U28's own capstone at the service level: a real
/// `security.suspend_process` grant really stops the target (unit
/// U27's own real path), then a genuinely *separate* HTTP rollback call
/// — knowing only the row id, nothing else — really resumes it. Before
/// this unit, nothing in the running product could do this at all.
#[tokio::test]
async fn granting_a_suspend_then_rolling_it_back_over_http_really_resumes_the_target() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "security.suspend_process",
        &target,
        json!({}),
    )
    .await;

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "grant body: {body:?}");

    let platform = platform::current_platform_adapter();
    assert!(
        platform
            .is_process_stopped(target.pid as u32)
            .expect("is_process_stopped after grant"),
        "the real process must be stopped after granting the suspend"
    );

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/rollback"),
        json!({ "rolled_back_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "rollback body: {body:?}");

    assert!(
        !platform
            .is_process_stopped(target.pid as u32)
            .expect("is_process_stopped after rollback"),
        "the real process must be resumed after the rollback — the exact \
         capability that was missing before this unit"
    );

    let row = repository::get_action(&repo, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(
        row.status, "EXECUTED",
        "the original row's own status must stay exactly as execute() left it"
    );
}

/// Unit U30's own capstone at the service level: `previous_state` is now
/// really persisted (migrations/V13), so a real `host.set_process_cpu_
/// affinity` grant, rolled back in a genuinely separate HTTP call, now
/// really restores the original mask — not just fails safely, closing
/// ADR 0033's own named gap end-to-end over real HTTP.
#[tokio::test]
async fn granting_an_affinity_change_then_rolling_it_back_over_http_really_restores_it() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "host.set_process_cpu_affinity",
        &target,
        json!({ "cpus": [0] }),
    )
    .await;

    let platform = platform::current_platform_adapter();
    let original = platform
        .get_process_cpu_affinity(target.pid as u32)
        .expect("read original affinity");

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "grant body: {body:?}");
    assert_eq!(
        platform
            .get_process_cpu_affinity(target.pid as u32)
            .expect("read affinity after grant")
            .cpus,
        std::collections::BTreeSet::from([0])
    );

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/rollback"),
        json!({ "rolled_back_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "rollback body: {body:?}");

    let after_rollback = platform
        .get_process_cpu_affinity(target.pid as u32)
        .expect("read affinity after rollback");
    assert_eq!(
        after_rollback, original,
        "the real affinity mask must be genuinely restored, not just fail safely"
    );
}

#[tokio::test]
async fn rolling_back_a_never_granted_action_is_a_real_bad_request() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();
    let row_id = insert_pending_for(
        &repo,
        "tester",
        "security.suspend_process",
        &target,
        json!({}),
    )
    .await;

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/rollback"),
        json!({ "rolled_back_by": "tester" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body:?}");
    assert_eq!(body["error"]["code"], "BAD_REQUEST");
}

#[tokio::test]
async fn rolling_back_a_nonexistent_action_is_not_found() {
    let repo = open_repo().await;
    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        "/api/v1/policy/999999/rollback",
        json!({ "rolled_back_by": "tester" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn granting_a_nonexistent_action_is_not_found() {
    let repo = open_repo().await;
    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        "/api/v1/policy/999999/grant",
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}
