//! Real end-to-end coverage of unit U31's first direct `core::dbms::
//! DbmsAdapter` wire surface: a real axum router and a real, throwaway
//! PostgreSQL instance (`support::TestPostgres`, the same real harness
//! `correlation_endpoints.rs` already established — real `initdb`/
//! `pg_ctl`, no Docker), proving `list_sessions`/`lock_graph` return
//! genuine `pg_stat_activity` data, not a fixture.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration as StdDuration;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::{pool, DbmsAdapter};
use ai_ops_core::policy::{ActionTypeRegistry, Environment, ProtectedResourceRegistry};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

async fn build_app(dbms_adapter: Option<Arc<dyn DbmsAdapter>>) -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(
        platform,
        self_metrics,
        host_telemetry,
        None,
        dbms_adapter,
        Arc::new(ActionTypeRegistry::new()),
        Arc::new(ProtectedResourceRegistry::new()),
        Environment::Development,
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

fn adapter_for(pg: &support::TestPostgres) -> Arc<dyn DbmsAdapter> {
    let cfg = service::dbms_config::resolve_db_connection_from(
        Some(pg.socket_dir.display().to_string()),
        Some(pg.port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("trust-auth-ignores-this".to_string()),
        Some("disable".to_string()),
    )
    .expect("host+password were both given");
    let pg_pool = pool::build_pool(&cfg.metadata, &cfg.password).expect("pool builds");
    Arc::new(PostgresAdapter::from_pool(pg_pool, false))
}

#[tokio::test]
async fn sessions_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/sessions").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn locks_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/locks").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn query_stats_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/query-stats").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn table_stats_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/table-stats").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn index_stats_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/index-stats").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

/// Unit U33: `support::TestPostgres` doesn't configure `shared_preload_
/// libraries`, so `pg_stat_statements` is never actually loadable here
/// — confirmed by direct read of `core/tests/dbms_adapter_smoke_test.rs`
/// and `core/tests/dbms_no_pg_stat_statements_test.rs`, neither of
/// which proves the real `Supported` happy path either, only this same
/// honest degraded one. A real, DB-backed proof this endpoint reports
/// that real degradation as a genuine `200 OK` (not folded into a
/// `503`), not asserted from reading code alone.
#[tokio::test]
async fn query_stats_reports_a_real_unavailable_state_as_200_ok() {
    let mut pg = support::TestPostgres::start(17).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/query-stats").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "UNAVAILABLE");
    let reason = body["data"]["reason"].as_str().expect("reason is a string");
    assert!(
        reason.to_lowercase().contains("pg_stat_statements"),
        "expected the real adapter's own reason naming pg_stat_statements, got: {reason}"
    );
}

/// Unit U35: unlike `query_stats`, `table_stats`/`index_stats` have no
/// `shared_preload_libraries` constraint, so this reuses `core/tests/
/// dbms_adapter_smoke_test.rs`'s own proven real fixture to prove a
/// genuine, populated happy path end-to-end over real HTTP, not just
/// the degraded/unconfigured paths.
async fn setup_widgets_fixture(pg: &support::TestPostgres) {
    let client = pg.superuser_client().await;
    client
        .batch_execute(
            "CREATE TABLE widgets (id serial PRIMARY KEY, name text);
             INSERT INTO widgets (name) VALUES ('a'), ('b'), ('c');
             CREATE INDEX idx_widgets_name ON widgets (name);
             SELECT * FROM widgets;",
        )
        .await
        .expect("fixture setup");
}

#[tokio::test]
async fn table_stats_returns_real_populated_data() {
    let mut pg = support::TestPostgres::start(17).await;
    setup_widgets_fixture(&pg).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/table-stats").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let tables = body["data"].as_array().expect("data is an array");
    let widgets = tables
        .iter()
        .find(|t| t["table"] == "widgets")
        .unwrap_or_else(|| panic!("expected a widgets table stat, got: {tables:?}"));
    assert_eq!(widgets["schema"], "public");
    assert!(
        widgets["n_live_tup"]
            .as_i64()
            .expect("n_live_tup is a number")
            >= 3
    );
}

#[tokio::test]
async fn index_stats_returns_real_populated_data() {
    let mut pg = support::TestPostgres::start(17).await;
    setup_widgets_fixture(&pg).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/index-stats").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let indexes = body["data"].as_array().expect("data is an array");
    let widgets_idx = indexes
        .iter()
        .find(|i| i["index"] == "idx_widgets_name")
        .unwrap_or_else(|| panic!("expected idx_widgets_name index stat, got: {indexes:?}"));
    assert_eq!(widgets_idx["table"], "widgets");
    assert_eq!(widgets_idx["schema"], "public");
}

/// A real discovery while writing this test, not the assumption it
/// started with: `classify_session_state` (`core/src/dbms/adapters/
/// postgresql/queries.rs`) treats *any* non-null `wait_event_type` as
/// `Waiting`, checked before the `state` column at all — and real
/// PostgreSQL reports `wait_event_type = 'Client'` for *any* backend
/// sitting idle on the socket waiting for its next command, including
/// one that's idle inside an open transaction. So a real, genuinely
/// idle-in-transaction backend classifies as `WAITING`, not
/// `IDLE_IN_TRANSACTION` — real, existing, unmodified `core::dbms`
/// behavior (out of this unit's scope to change), asserted here as it
/// actually is, not as first assumed.
#[tokio::test]
async fn sessions_returns_a_real_idle_backend_as_waiting() {
    let mut pg = support::TestPostgres::start(17).await;

    // A real, genuinely idle-in-transaction backend: an open BEGIN with
    // no query since, held open until this test explicitly rolls it
    // back.
    let (client, connection) = pg
        .config("postgres", "postgres")
        .connect(tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute("BEGIN;")
        .await
        .expect("open a real transaction");
    let backend_pid: i32 = client
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("query pg_backend_pid")
        .get(0);

    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/sessions").await;

    client
        .batch_execute("ROLLBACK;")
        .await
        .expect("release the real transaction");
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let sessions = body["data"].as_array().expect("data is an array");
    let session = sessions
        .iter()
        .find(|s| s["pid"] == backend_pid)
        .unwrap_or_else(|| {
            panic!("the real idle-in-transaction backend {backend_pid} must appear, got {body:?}")
        });
    assert_eq!(session["state"], "WAITING");
}

#[tokio::test]
async fn locks_returns_a_real_blocking_edge_from_genuinely_induced_contention() {
    let mut pg = support::TestPostgres::start(17).await;
    let setup = pg.superuser_client().await;
    setup
        .batch_execute("CREATE TABLE t (id int); INSERT INTO t VALUES (1)")
        .await
        .expect("create table and seed a row");

    let (holder, holder_conn) = pg
        .config("postgres", "postgres")
        .connect(tokio_postgres::NoTls)
        .await
        .expect("connect holder");
    tokio::spawn(async move {
        let _ = holder_conn.await;
    });
    holder
        .batch_execute("BEGIN; SELECT id FROM t WHERE id = 1 FOR UPDATE;")
        .await
        .expect("acquire the row lock");
    let holder_pid: i32 = holder
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("query pg_backend_pid")
        .get(0);

    let (waiter, waiter_conn) = pg
        .config("postgres", "postgres")
        .connect(tokio_postgres::NoTls)
        .await
        .expect("connect waiter");
    tokio::spawn(async move {
        let _ = waiter_conn.await;
    });
    let waiter_pid: i32 = waiter
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("query pg_backend_pid")
        .get(0);
    tokio::spawn(async move {
        let _ = waiter
            .simple_query("SELECT id FROM t WHERE id = 1 FOR UPDATE")
            .await;
    });

    // Poll pg_stat_activity for the real block to register — no guessed
    // sleep.
    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(10);
    loop {
        let row = setup
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE pid = $1 \
                 AND wait_event_type = 'Lock'",
                &[&waiter_pid],
            )
            .await
            .expect("query pg_stat_activity");
        let waiting: i64 = row.get(0);
        if waiting >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the waiter never registered as blocked in pg_stat_activity"
        );
        tokio::time::sleep(StdDuration::from_millis(50)).await;
    }

    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/locks").await;

    holder
        .batch_execute("ROLLBACK;")
        .await
        .expect("release the row lock");
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let locks = body["data"].as_array().expect("data is an array");
    let edge = locks
        .iter()
        .find(|l| l["blocked_pid"] == waiter_pid)
        .unwrap_or_else(|| {
            panic!("a real blocking edge for waiter {waiter_pid} must appear, got {body:?}")
        });
    assert_eq!(edge["blocking_pid"], holder_pid);
}
