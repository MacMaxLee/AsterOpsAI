//! Real end-to-end coverage of the security triage view (unit U15): a
//! real axum router (`tower::ServiceExt::oneshot`), a real SQLite-backed
//! `RepositoryHandle`, real events recorded via `ai_ops_core::security::
//! incident::record_event` directly (proving this endpoint's own
//! listing/mapping/count, not re-proving the detector functions
//! themselves, which `core`'s own test suite already covers).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ai_ops_core::policy::{
    ActionTypeRegistry, Environment, ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use ai_ops_core::repository::{self, RepositoryConfig};
use ai_ops_core::security::{incident, DetectedEvent, Severity};
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

fn resource(name: &str) -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Device,
        name: name.to_string(),
    }
}

fn event(detector_id: &'static str, resource_name: &str) -> DetectedEvent {
    DetectedEvent {
        detector_id,
        severity: Severity::Medium,
        category: "TEST",
        summary: "a test event".to_string(),
        evidence_json: "{}".to_string(),
        resource: resource(resource_name),
        ts: Utc::now(),
    }
}

#[tokio::test]
async fn an_empty_repository_has_no_open_incidents() {
    let repo = open_repo().await;
    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/security/incidents").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], json!([]));
}

#[tokio::test]
async fn a_second_correlated_event_increases_the_same_incidents_event_count() {
    let repo = open_repo().await;
    incident::record_event(&repo, event("test.detector", "sdb"))
        .await
        .expect("record_event 1");

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = get_json(app, "/api/v1/security/incidents").await;
    assert_eq!(status, StatusCode::OK);
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["event_count"], 1);
    assert_eq!(items[0]["status"], "OPEN");
    let incident_id = items[0]["id"].as_i64().expect("id");

    incident::record_event(&repo, event("test.detector", "sdb"))
        .await
        .expect("record_event 2");

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, "/api/v1/security/incidents").await;
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(
        items.len(),
        1,
        "a correlated second event must not open a second incident"
    );
    assert_eq!(items[0]["id"], incident_id);
    assert_eq!(items[0]["event_count"], 2);
}

/// Unit U78 (ADR 0023's own named gap, closed by docs/adr/0083): an
/// incident's own `detector_id`/resource must be visible on the wire —
/// not just an event count — so a console per-incident suppress action
/// has something real to suppress.
#[tokio::test]
async fn an_incident_summary_carries_its_own_detector_id_and_resource() {
    let repo = open_repo().await;
    incident::record_event(&repo, event("test.detector", "sdb"))
        .await
        .expect("record_event");

    let app = build_app(Some(repo)).await;
    let (status, body) = get_json(app, "/api/v1/security/incidents").await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["detector_id"], "test.detector");
    assert_eq!(items[0]["resource_kind"], "DEVICE");
    assert_eq!(items[0]["resource_name"], "sdb");
}

/// Unit U76 (ADR 0016/0020's own named gap, resolved to manual-only —
/// docs/adr/0081): closing an incident sets it to `CLOSED` (real
/// `closed_at`) and it must stop appearing in the open-incidents list.
#[tokio::test]
async fn closing_an_open_incident_marks_it_closed_and_removes_it_from_the_open_list() {
    let repo = open_repo().await;
    incident::record_event(&repo, event("test.detector", "sdb"))
        .await
        .expect("record_event");

    let app = build_app(Some(repo.clone())).await;
    let (_, body) = get_json(app, "/api/v1/security/incidents").await;
    let incident_id = body["data"][0]["id"].as_i64().expect("id");

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/security/incidents/{incident_id}/close"),
        json!({ "closed_by": "tester" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["status"], "CLOSED");
    assert!(
        !body["data"]["closed_at"].is_null(),
        "closed_at must be set: {body:?}"
    );
    assert_eq!(
        body["data"]["detector_id"], "test.detector",
        "a closed incident's summary must still carry its own detector_id/resource: {body:?}"
    );
    assert_eq!(body["data"]["resource_name"], "sdb");

    assert_eq!(
        repository::latest_audit_event_type(&repo)
            .await
            .expect("latest_audit_event_type"),
        Some("security.incident_closed".to_string()),
        "closing an incident must leave a real audit trail, same as policy grant/reject"
    );

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, "/api/v1/security/incidents").await;
    assert_eq!(
        body["data"],
        json!([]),
        "a closed incident must no longer appear in the open-incidents list"
    );
}

/// Closing an already-`CLOSED` incident is a harmless, idempotent
/// no-op — it must not overwrite the original `closed_at`.
#[tokio::test]
async fn closing_an_already_closed_incident_is_idempotent() {
    let repo = open_repo().await;
    incident::record_event(&repo, event("test.detector", "sdb"))
        .await
        .expect("record_event");
    let app = build_app(Some(repo.clone())).await;
    let (_, body) = get_json(app, "/api/v1/security/incidents").await;
    let incident_id = body["data"][0]["id"].as_i64().expect("id");

    let app = build_app(Some(repo.clone())).await;
    let (status, first) = post_json(
        app,
        &format!("/api/v1/security/incidents/{incident_id}/close"),
        json!({ "closed_by": "tester" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let first_closed_at = first["data"]["closed_at"].clone();

    let app = build_app(Some(repo)).await;
    let (status, second) = post_json(
        app,
        &format!("/api/v1/security/incidents/{incident_id}/close"),
        json!({ "closed_by": "tester-again" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {second:?}");
    assert_eq!(second["data"]["status"], "CLOSED");
    assert_eq!(
        second["data"]["closed_at"], first_closed_at,
        "re-closing an already-closed incident must not overwrite its original closed_at"
    );
}

#[tokio::test]
async fn closing_a_nonexistent_incident_returns_not_found() {
    let repo = open_repo().await;
    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        "/api/v1/security/incidents/999999/close",
        json!({ "closed_by": "tester" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body:?}");
}

#[tokio::test]
async fn suppressing_a_detector_and_resource_blocks_only_that_pair() {
    let repo = open_repo().await;

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        "/api/v1/security/suppress",
        json!({
            "detector_id": "test.detector",
            "resource": { "kind": "DEVICE", "name": "sdb" },
            "reason": "known false positive",
            "created_by": "tester"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");

    let outcome = incident::record_event(&repo, event("test.detector", "sdb"))
        .await
        .expect("record_event");
    assert!(
        matches!(outcome, incident::IncidentOutcome::Suppressed),
        "suppress() must reach record_event's own real suppression check, not just insert a row"
    );

    let other = incident::record_event(&repo, event("test.detector", "sdc"))
        .await
        .expect("record_event");
    assert!(
        matches!(other, incident::IncidentOutcome::Recorded { .. }),
        "a resource-scoped suppression must not suppress a different resource"
    );

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, "/api/v1/security/incidents").await;
    let items = body["data"].as_array().expect("data is an array");
    assert_eq!(
        items.len(),
        1,
        "only the non-suppressed resource's incident should exist"
    );
}
