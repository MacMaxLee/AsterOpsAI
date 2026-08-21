//! Real end-to-end coverage of the tuning plan history view (unit U14)
//! and the real `POST /tuning/start` (unit U23): a real axum router
//! (`tower::ServiceExt::oneshot`), a real SQLite-backed
//! `RepositoryHandle`, and — for the start-plan tests — a real spawned
//! process as the tuning target, so starting a plan genuinely runs
//! `core::tuning::start_plan` against something real. One test
//! (`ask_before_changes_creates_a_real_row_the_existing_inbox_cannot_
//! yet_reach`) documents a real, discovered boundary rather than a happy
//! path: see its own doc comment and docs/adr/0028.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use ai_ops_core::policy::{Environment, ProtectedResourceRegistry};
use ai_ops_core::repository::{self, NewTuningPlan, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use service::{actions::build_action_registry, api, self_metrics, state::AppState, telemetry};
use support::SpawnedTarget;
use tower::ServiceExt;

async fn build_app(repository: Option<repository::RepositoryHandle>) -> axum::Router {
    build_app_with_environment(repository, Environment::Development).await
}

/// Unit U25: lets a test configure the exact `policy_environment`
/// `AppState` was actually started with, rather than always the
/// no-behavior-change `Development` default `build_app` uses — needed
/// to prove the propose -> inbox -> grant -> execute loop is genuinely
/// reachable once a deployment is really configured for `Production`.
async fn build_app_with_environment(
    repository: Option<repository::RepositoryHandle>,
    policy_environment: Environment,
) -> axum::Router {
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
        policy_environment,
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

fn start_request(target: &SpawnedTarget, profile: &str, mode: &str) -> Value {
    json!({
        "pid": target.pid,
        "start_time_ticks": target.start_time_ticks,
        "resource_name": "sleep-test-target",
        "profile": profile,
        "mode": mode,
        "requested_by": "tester",
    })
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

#[tokio::test]
async fn recommend_only_returns_real_candidates_and_touches_nothing_else() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();

    // BATTERY_SAVER wants BelowNormal priority + affinity {0} — a real,
    // freshly-spawned process starts at Normal priority with the full
    // CPU set, so both fields genuinely differ and both get proposed.
    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        "/api/v1/tuning/start",
        start_request(&target, "BATTERY_SAVER", "RECOMMEND_ONLY"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["status"], "COMPLETED");
    let candidates = body["data"]["candidates"].as_array().expect("candidates");
    assert_eq!(
        candidates.len(),
        2,
        "both priority and affinity should differ"
    );
    for candidate in candidates {
        assert_eq!(candidate["outcome"], "RECOMMENDED");
        assert!(
            candidate["row_id"].is_null(),
            "RECOMMEND_ONLY must never touch the policy engine, so no row is ever proposed"
        );
    }

    let app = build_app(Some(repo)).await;
    let (_, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(
        body["data"],
        json!([]),
        "a pure recommendation must never appear in the policy inbox"
    );
}

/// A real, honest architectural boundary discovered while wiring this
/// endpoint: with only Low-risk action types registered (unit U22) and
/// `Environment::Development` hardcoded (this unit),
/// `core::policy::risk::decide` always resolves Low risk to `AutoAllow`
/// in Development — so `ASK_BEFORE_CHANGES` can never actually produce a
/// `PENDING_APPROVAL` candidate through this endpoint today; every
/// candidate lands on `AUTO_ALLOWED_PENDING` instead (matching core's own
/// `tuning_plan_recommend_and_ask_test.rs::ask_before_changes_proposes_
/// for_real_but_never_authorizes`). That row is real, audited, and has a
/// real `row_id` — but it is genuinely orphaned from every existing HTTP
/// surface: `list_pending_approval` only selects `status =
/// 'PENDING_APPROVAL'`, so it never appears in `/api/v1/policy/pending`,
/// and unit U22's grant endpoint's first step (`approval::grant`) has its
/// own CAS require `PENDING_APPROVAL`, so granting it is a real, correct
/// `400` — not a bug in this endpoint, and not something this unit's
/// scope (pure wiring, no `core::policy` changes) closes. See
/// docs/adr/0028's "known gap" section: the full propose -> inbox ->
/// grant -> execute loop is not reachable over HTTP for any action type
/// registered today, and won't be until either a Medium/High-risk action
/// type exists or `Environment` stops being hardcoded to `Development`.
#[tokio::test]
async fn ask_before_changes_creates_a_real_row_the_existing_inbox_cannot_yet_reach() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();

    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        "/api/v1/tuning/start",
        start_request(&target, "BATTERY_SAVER", "ASK_BEFORE_CHANGES"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let candidates = body["data"]["candidates"].as_array().expect("candidates");
    assert_eq!(candidates.len(), 2);
    for candidate in candidates {
        assert_eq!(candidate["outcome"], "AUTO_ALLOWED_PENDING");
        assert!(!candidate["row_id"].is_null());
    }
    let affinity_row_id = candidates
        .iter()
        .find(|c| c["action_type"] == "host.set_process_cpu_affinity")
        .expect("an affinity candidate was proposed")["row_id"]
        .as_i64()
        .expect("row_id is an integer");

    // Real, audited row — but genuinely invisible to the inbox: Low risk
    // auto-allows in Development, so it never becomes PENDING_APPROVAL.
    let app = build_app(Some(repo.clone())).await;
    let (_, body) = get_json(app, "/api/v1/policy/pending").await;
    assert_eq!(
        body["data"],
        json!([]),
        "an AUTO_ALLOWED_PENDING row must not appear in the PENDING_APPROVAL-only inbox"
    );

    // The existing grant endpoint genuinely cannot reach it either — a
    // real, correct 400, not a panic or a silent no-op.
    let app = build_app(Some(repo.clone())).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{affinity_row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body:?}");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("message is a string")
            .contains("AUTO_ALLOWED"),
        "expected the real status-mismatch message naming the row's actual status, got {body:?}"
    );

    // Real proof nothing on the live target changed either.
    let platform = platform::current_platform_adapter();
    let affinity = platform
        .get_process_cpu_affinity(target.pid as u32)
        .expect("get_process_cpu_affinity");
    assert_ne!(
        affinity.cpus,
        std::collections::BTreeSet::from([0]),
        "an AUTO_ALLOWED_PENDING row must never have touched live state"
    );
}

/// Unit U25: the exact loop ADR 0028 documented as unreachable, proven
/// reachable, live over real HTTP, once the deployment is genuinely
/// configured for `Environment::Production` — `core::policy::risk::
/// decide(Low, Production)` requires approval (confirmed by direct read
/// of `core/src/policy/risk.rs`), so the same `ASK_BEFORE_CHANGES` plan
/// that lands on `AUTO_ALLOWED_PENDING` under the `Development` default
/// (see the test above) now genuinely produces a real `PENDING_APPROVAL`
/// row instead — visible in the inbox, grantable, and really executed.
#[tokio::test]
async fn environment_production_makes_ask_before_changes_really_reach_the_inbox_and_grantable() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();

    let app = build_app_with_environment(Some(repo.clone()), Environment::Production).await;
    let (status, body) = post_json(
        app,
        "/api/v1/tuning/start",
        start_request(&target, "BATTERY_SAVER", "ASK_BEFORE_CHANGES"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let candidates = body["data"]["candidates"].as_array().expect("candidates");
    assert_eq!(candidates.len(), 2);
    for candidate in candidates {
        assert_eq!(candidate["outcome"], "PENDING_APPROVAL");
        assert!(!candidate["row_id"].is_null());
    }
    let affinity_row_id = candidates
        .iter()
        .find(|c| c["action_type"] == "host.set_process_cpu_affinity")
        .expect("an affinity candidate was proposed")["row_id"]
        .as_i64()
        .expect("row_id is an integer");

    // Genuinely visible in the real Policy inbox this time.
    let app = build_app_with_environment(Some(repo.clone()), Environment::Production).await;
    let (_, body) = get_json(app, "/api/v1/policy/pending").await;
    let pending_ids: Vec<i64> = body["data"]
        .as_array()
        .expect("data is an array")
        .iter()
        .map(|item| item["id"].as_i64().expect("id is an integer"))
        .collect();
    assert!(
        pending_ids.contains(&affinity_row_id),
        "the proposed affinity candidate must be visible in the real inbox: {pending_ids:?}"
    );

    // Grant it for real (unit U22's own authorize -> execute path) —
    // this genuinely succeeds now, unlike the Development-default test.
    let app = build_app_with_environment(Some(repo), Environment::Production).await;
    let (status, body) = post_json(
        app,
        &format!("/api/v1/policy/{affinity_row_id}/grant"),
        json!({ "granted_by": "approver" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "grant body: {body:?}");

    // Real proof: the target's actual CPU affinity changed.
    let platform = platform::current_platform_adapter();
    let affinity = platform
        .get_process_cpu_affinity(target.pid as u32)
        .expect("get_process_cpu_affinity");
    assert_eq!(
        affinity.cpus,
        std::collections::BTreeSet::from([0]),
        "the real process affinity must now be exactly {{0}}, proving the full \
         propose -> inbox -> grant -> execute loop ran for real"
    );
}

#[tokio::test]
async fn starting_a_second_plan_for_an_in_flight_target_is_a_real_bad_request() {
    let repo = open_repo().await;
    let target = SpawnedTarget::spawn();

    // A real IN_FLIGHT row for this exact target, inserted directly —
    // the same real partial UNIQUE index (migrations/V11) `start_plan`
    // itself would hit if a second real call raced the first.
    let new = NewTuningPlan {
        created_at: Utc::now(),
        target_identity_json: target.target_identity_json(),
        target_start_time: target.start_time_ticks,
        profile: "BALANCED".to_string(),
        mode: "ASK_BEFORE_CHANGES".to_string(),
        status: "IN_FLIGHT".to_string(),
        candidates_json: "[]".to_string(),
    };
    repository::insert_tuning_plan(&repo, new)
        .await
        .expect("insert the in-flight plan directly");

    let app = build_app(Some(repo)).await;
    let (status, body) = post_json(
        app,
        "/api/v1/tuning/start",
        start_request(&target, "BALANCED", "RECOMMEND_ONLY"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body:?}");
    assert_eq!(body["success"], false);
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("message is a string")
            .contains("already in flight"),
        "expected a real 'already in flight' message, got {body:?}"
    );
}
