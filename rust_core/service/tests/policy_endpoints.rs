//! Real end-to-end coverage of the policy approval inbox (unit U13): a
//! real axum router (`tower::ServiceExt::oneshot` — real requests, real
//! JSON, no network listener needed), a real SQLite-backed
//! `RepositoryHandle`, real rows inserted via `repository::propose_action`
//! directly (proving this endpoint's own listing/mapping/transition
//! logic, not re-proving `policy::evaluate`'s decision logic, which
//! `core`'s own test suite already covers).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ai_ops_core::policy::{ActionStatus, ResourceDescriptor, ResourceKind};
use ai_ops_core::repository::{self, NewProposedAction, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use serde_json::{json, Value};
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

async fn build_app(repository: Option<repository::RepositoryHandle>) -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), repository.clone());
    let state = AppState::new(platform, self_metrics, host_telemetry, repository);
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

async fn insert_pending(repo: &repository::RepositoryHandle, requested_by: &str) -> i64 {
    let now = Utc::now();
    let new = NewProposedAction {
        created_at: now,
        action_type: "host.set_process_priority".to_string(),
        target_identity_json: serde_json::json!({
            "kind": "PROCESS", "pid": 999_999, "start_time_ticks": 1
        })
        .to_string(),
        target_start_time: 1,
        risk_classification: "MEDIUM".to_string(),
        status: ActionStatus::PendingApproval.as_str().to_string(),
        evidence_json: "{}".to_string(),
        requested_by: requested_by.to_string(),
        parameters_json: "{}".to_string(),
        parameters_hash: "hash".to_string(),
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
    let row_id = insert_pending(&repo, "tester").await;

    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(status, StatusCode::OK);
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["id"], row_id);
    assert_eq!(item["action_type"], "host.set_process_priority");
    assert_eq!(item["risk_classification"], "MEDIUM");
    assert_eq!(item["resource_kind"], "PROCESS");
    assert_eq!(item["resource_name"], "sleep-test-target");
    assert_eq!(item["requested_by"], "tester");
    assert!(!item["approval_expires_at"].is_null());
}

#[tokio::test]
async fn granting_a_pending_action_removes_it_from_the_inbox() {
    let repo = open_repo().await;
    let row_id = insert_pending(&repo, "tester").await;

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");

    let row = repository::get_action(&repo, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "APPROVED");
    assert_eq!(row.approved_by.as_deref(), Some("approver"));

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(
        body["data"],
        json!([]),
        "a granted action must leave the inbox"
    );
}

#[tokio::test]
async fn rejecting_a_pending_action_records_a_distinct_audit_event() {
    let repo = open_repo().await;
    let row_id = insert_pending(&repo, "tester").await;

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
    let row_id = insert_pending(&repo, "tester").await;

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
