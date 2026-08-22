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

fn adapter_for(pg: &support::TestPostgres) -> Arc<dyn DbmsAdapter> {
    let cfg = service::dbms_config::resolve_db_connection_from(
        Some(pg.socket_dir.display().to_string()),
        Some(pg.port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("trust-auth-ignores-this".to_string()),
        Some("disable".to_string()),
        None,
        None,
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

#[tokio::test]
async fn replication_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/replication").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn gucs_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/gucs").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn temp_file_activity_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/temp-file-activity").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn deadlock_history_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/deadlock-history").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn long_transactions_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/long-transactions").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn idle_in_transaction_sessions_is_unavailable_without_a_database() {
    let app = build_app(None).await;
    let (status, body) = get_json(app, "/api/v1/dbms/idle-in-transaction-sessions").await;
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

/// Unit U51 (ADR 0038/0056): closes the gap
/// `query_stats_reports_a_real_unavailable_state_as_200_ok`'s own doc
/// comment named — `pg_stat_statements` genuinely loadable this time,
/// via `TestPostgres::start_with_extra_options`'s new
/// `shared_preload_libraries` support. Runs a real, distinctively-named
/// parameterized query 3 times so `pg_stat_statements` normalizes it to
/// a single real row with `calls >= 3`.
async fn setup_query_stats_fixture(pg: &support::TestPostgres) {
    let client = pg.superuser_client().await;
    client
        .batch_execute(
            "CREATE EXTENSION pg_stat_statements;
             CREATE TABLE widgets_for_query_stats (id serial PRIMARY KEY, name text);
             INSERT INTO widgets_for_query_stats (name) VALUES ('a')",
        )
        .await
        .expect("fixture setup");
    for _ in 0..3 {
        client
            .query(
                "SELECT name FROM widgets_for_query_stats WHERE id = $1",
                &[&1i32],
            )
            .await
            .expect("run tracked query");
    }
}

#[tokio::test]
async fn query_stats_returns_real_populated_data() {
    let mut pg = support::TestPostgres::start_with_extra_options(
        17,
        "-c shared_preload_libraries=pg_stat_statements",
    )
    .await;
    setup_query_stats_fixture(&pg).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/query-stats").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "SUPPORTED");
    let stats = body["data"]["value"].as_array().expect("value is an array");
    let tracked = stats
        .iter()
        .find(|s| {
            let query = s["normalized_query"].as_str().unwrap_or("");
            // `.contains("widgets_for_query_stats")` alone also matches the
            // fixture's own `CREATE TABLE`/`INSERT` statements (a real
            // discovery — `pg_stat_statements` tracks every statement, not
            // just the one this test cares about) — `starts_with("SELECT")`
            // narrows to the specific, repeatedly-run tracked query.
            query.starts_with("SELECT") && query.contains("widgets_for_query_stats")
        })
        .unwrap_or_else(|| {
            panic!("expected the tracked SELECT naming widgets_for_query_stats, got: {stats:?}")
        });
    assert!(
        tracked["calls"].as_i64().expect("calls is a number") >= 3,
        "expected at least 3 real calls, got: {tracked:?}"
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

/// Unit U37: reuses `core/tests/dbms_adapter_smoke_test.rs`'s own real
/// assertion — an ordinary, non-replica `TestPostgres` instance
/// genuinely reports `is_primary: true`, `in_recovery: false`, and an
/// empty `standbys` list. No standby-pair fixture exists in this
/// project's test harness (real, separate future capability), so the
/// standby-populated path remains genuinely unverified — same honest
/// gap-naming discipline as U33's `query_stats` `Unavailable`-only proof.
#[tokio::test]
async fn replication_status_reports_a_real_primary_state() {
    let mut pg = support::TestPostgres::start(17).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/replication").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["is_primary"], true);
    assert_eq!(body["data"]["in_recovery"], false);
    assert_eq!(
        body["data"]["standbys"]
            .as_array()
            .expect("standbys is an array")
            .len(),
        0
    );
}

/// Unit U52 (ADR 0042/0057): closes the last remaining test-coverage
/// gap named above — a real streaming-replication standby, built via
/// `support::TestPostgres::start_standby_of`'s real `pg_basebackup`,
/// proving both the primary's real `pg_stat_replication`-backed view
/// and the standby's own in-recovery view, neither ever exercised over
/// HTTP before this unit.
#[tokio::test]
async fn replication_status_reports_a_real_standby_once_streaming() {
    let mut primary = support::TestPostgres::start(17).await;
    let mut standby = primary.start_standby_of(17).await;

    // Poll the primary's real pg_stat_replication for the real standby
    // to register — no guessed sleep.
    let setup = primary.superuser_client().await;
    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(15);
    loop {
        let row = setup
            .query_one("SELECT count(*) FROM pg_stat_replication", &[])
            .await
            .expect("query pg_stat_replication");
        let count: i64 = row.get(0);
        if count >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the real standby never registered in pg_stat_replication"
        );
        tokio::time::sleep(StdDuration::from_millis(100)).await;
    }

    let primary_adapter = adapter_for(&primary);
    let primary_app = build_app(Some(primary_adapter)).await;
    let (primary_status, primary_body) = get_json(primary_app, "/api/v1/dbms/replication").await;

    let standby_adapter = adapter_for(&standby);
    let standby_app = build_app(Some(standby_adapter)).await;
    let (standby_status, standby_body) = get_json(standby_app, "/api/v1/dbms/replication").await;

    standby.stop().await;
    primary.stop().await;

    assert_eq!(primary_status, StatusCode::OK, "body: {primary_body:?}");
    assert_eq!(primary_body["data"]["is_primary"], true);
    assert_eq!(primary_body["data"]["in_recovery"], false);
    let standbys = primary_body["data"]["standbys"]
        .as_array()
        .expect("standbys is an array");
    assert_eq!(
        standbys.len(),
        1,
        "expected exactly one real streaming standby, got: {standbys:?}"
    );
    assert_eq!(standbys[0]["state"], "streaming");

    assert_eq!(
        standby_status,
        StatusCode::OK,
        "standby body: {standby_body:?}"
    );
    assert_eq!(standby_body["data"]["is_primary"], false);
    assert_eq!(standby_body["data"]["in_recovery"], true);
    assert_eq!(
        standby_body["data"]["standbys"]
            .as_array()
            .expect("standbys is an array")
            .len(),
        0
    );
}

#[tokio::test]
async fn gucs_returns_real_settings_including_max_connections() {
    let mut pg = support::TestPostgres::start(17).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/gucs").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let gucs = body["data"].as_array().expect("data is an array");
    let max_conn = gucs
        .iter()
        .find(|g| g["name"] == "max_connections")
        .unwrap_or_else(|| panic!("expected a real max_connections guc, got: {gucs:?}"));
    assert!(
        !max_conn["setting"]
            .as_str()
            .expect("setting is a string")
            .is_empty(),
        "expected a real, non-empty setting value"
    );
}

/// Unit U49 (ADR 0054): `classify_session_state` used to treat *any*
/// non-null `wait_event_type` as `Waiting` — and real PostgreSQL
/// reports `wait_event_type = 'Client'` for *any* backend sitting idle
/// on the socket waiting for its next command, including one that's
/// idle inside an open transaction, so a genuinely idle-in-transaction
/// backend used to misclassify as `WAITING`. Fixed to check
/// specifically for `'Lock'` (see the corrected function's own doc
/// comment); this test now asserts the correct classification.
#[tokio::test]
async fn sessions_returns_a_real_idle_in_transaction_backend_correctly() {
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
    assert_eq!(session["state"], "IDLE_IN_TRANSACTION");
}

/// Also proves the U49/ADR 0054 fix's real contrast: a genuine
/// `wait_event_type = 'Lock'` waiter classifies as `WAITING` in
/// `/api/v1/dbms/sessions`, distinct from the genuine idle-in-
/// transaction case `sessions_returns_a_real_idle_in_transaction_
/// backend_correctly` covers.
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
    let (status, body) = get_json(app.clone(), "/api/v1/dbms/locks").await;
    let (sessions_status, sessions_body) = get_json(app, "/api/v1/dbms/sessions").await;

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

    assert_eq!(sessions_status, StatusCode::OK, "body: {sessions_body:?}");
    let sessions = sessions_body["data"].as_array().expect("data is an array");
    let waiter_session = sessions
        .iter()
        .find(|s| s["pid"] == waiter_pid)
        .unwrap_or_else(|| {
            panic!("the real lock waiter {waiter_pid} must appear, got {sessions_body:?}")
        });
    assert_eq!(waiter_session["state"], "WAITING");
    assert_eq!(edge["blocking_pid"], holder_pid);
}

/// Unit U39: unlike `core/tests/dbms_adapter_smoke_test.rs`'s own
/// trivially-true `temp.temp_files >= 0` on an idle fixture, this
/// genuinely forces an external sort past a deliberately tiny
/// `work_mem` so `pg_stat_database.temp_files`/`temp_bytes` really
/// increment — the same "poll real state, not a guessed sleep"
/// discipline `locks_returns_a_real_blocking_edge_from_genuinely_
/// induced_contention` already established.
#[tokio::test]
async fn temp_file_activity_reports_a_real_spill_to_disk() {
    let mut pg = support::TestPostgres::start(17).await;
    let setup = pg.superuser_client().await;
    setup
        .execute("SET work_mem = '64kB'", &[])
        .await
        .expect("set work_mem");
    let rows = setup
        .query(
            "SELECT g FROM generate_series(1, 200000) g ORDER BY random()",
            &[],
        )
        .await
        .expect("force a real external sort spill");
    assert_eq!(rows.len(), 200_000);

    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(10);
    loop {
        let row = setup
            .query_one(
                "SELECT temp_files FROM pg_stat_database WHERE datname = current_database()",
                &[],
            )
            .await
            .expect("query pg_stat_database");
        let temp_files: i64 = row.get(0);
        if temp_files >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "temp_files counter never incremented after a real spill"
        );
        tokio::time::sleep(StdDuration::from_millis(50)).await;
    }

    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/temp-file-activity").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let temp_files = body["data"]["temp_files"]
        .as_i64()
        .expect("temp_files is a number");
    let temp_bytes = body["data"]["temp_bytes"]
        .as_i64()
        .expect("temp_bytes is a number");
    assert!(
        temp_files >= 1,
        "expected a real spill to disk, got temp_files={temp_files}"
    );
    assert!(
        temp_bytes > 0,
        "expected real nonzero temp bytes, got temp_bytes={temp_bytes}"
    );
}

/// Unit U39: unlike the smoke test's trivially-true `deadlocks == 0` on
/// an idle fixture, this genuinely induces a real two-connection
/// deadlock (each locks one row, then requests the other's, with
/// `deadlock_timeout` lowered so PostgreSQL's own detector fires
/// quickly) so `pg_stat_database.deadlocks` really increments — reusing
/// the exact two-connection/poll-not-sleep technique the locks test
/// above already established.
#[tokio::test]
async fn deadlock_history_reports_a_real_induced_deadlock() {
    let mut pg = support::TestPostgres::start(17).await;
    let setup = pg.superuser_client().await;
    setup
        .batch_execute("CREATE TABLE dl (id int PRIMARY KEY); INSERT INTO dl VALUES (1), (2);")
        .await
        .expect("create table and seed two rows");

    let (conn_a, conn_a_driver) = pg
        .config("postgres", "postgres")
        .connect(tokio_postgres::NoTls)
        .await
        .expect("connect conn_a");
    tokio::spawn(async move {
        let _ = conn_a_driver.await;
    });
    conn_a
        .batch_execute(
            "SET deadlock_timeout = '50ms'; BEGIN; SELECT id FROM dl WHERE id = 1 FOR UPDATE;",
        )
        .await
        .expect("conn_a locks row 1");
    let conn_a_pid: i32 = conn_a
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("query pg_backend_pid")
        .get(0);

    let (conn_b, conn_b_driver) = pg
        .config("postgres", "postgres")
        .connect(tokio_postgres::NoTls)
        .await
        .expect("connect conn_b");
    tokio::spawn(async move {
        let _ = conn_b_driver.await;
    });
    conn_b
        .batch_execute(
            "SET deadlock_timeout = '50ms'; BEGIN; SELECT id FROM dl WHERE id = 2 FOR UPDATE;",
        )
        .await
        .expect("conn_b locks row 2");

    // conn_a now requests row 2 (held by conn_b) — blocks. Run in the
    // background so the main task can complete the cycle from conn_b.
    tokio::spawn(async move {
        let _ = conn_a
            .simple_query("SELECT id FROM dl WHERE id = 2 FOR UPDATE")
            .await;
    });

    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(10);
    loop {
        let row = setup
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE pid = $1 \
                 AND wait_event_type = 'Lock'",
                &[&conn_a_pid],
            )
            .await
            .expect("query pg_stat_activity");
        let waiting: i64 = row.get(0);
        if waiting >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "conn_a never registered as blocked on row 2"
        );
        tokio::time::sleep(StdDuration::from_millis(50)).await;
    }

    // conn_b now requests row 1 (held by conn_a) — completes the cycle;
    // PostgreSQL's own deadlock detector aborts one side after
    // deadlock_timeout.
    let _ = conn_b
        .simple_query("SELECT id FROM dl WHERE id = 1 FOR UPDATE")
        .await;

    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(10);
    loop {
        let row = setup
            .query_one(
                "SELECT deadlocks FROM pg_stat_database WHERE datname = current_database()",
                &[],
            )
            .await
            .expect("query pg_stat_database");
        let deadlocks: i64 = row.get(0);
        if deadlocks >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "deadlocks counter never incremented after a real induced deadlock"
        );
        tokio::time::sleep(StdDuration::from_millis(50)).await;
    }

    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/deadlock-history").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let deadlocks = body["data"]["deadlocks"]
        .as_i64()
        .expect("deadlocks is a number");
    assert!(
        deadlocks >= 1,
        "expected a real induced deadlock, got deadlocks={deadlocks}"
    );
}

/// Unit U41: both `long_transactions`/`idle_in_transaction_sessions`
/// filter on a hardcoded 60-second default threshold at the `core`
/// level (`LONG_TRANSACTION_THRESHOLD_SECONDS`/`IDLE_IN_TRANSACTION_
/// THRESHOLD_SECONDS`) — unlike U39's temp-file/deadlock pair, genuinely
/// exceeding that threshold would mean this test suite waits a real 60+
/// seconds on every single run, forever. This test proves the real,
/// honest empty path against the real default threshold; the populated
/// path below (unit U53) proves the real fixture instead, via
/// `PostgresAdapter::with_activity_thresholds`, not by waiting.
#[tokio::test]
async fn long_transactions_reports_a_real_empty_result() {
    let mut pg = support::TestPostgres::start(17).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/long-transactions").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let txns = body["data"].as_array().expect("data is an array");
    assert!(
        txns.is_empty(),
        "expected no real long-running transaction in a fresh fixture, got: {txns:?}"
    );
}

#[tokio::test]
async fn idle_in_transaction_sessions_reports_a_real_empty_result() {
    let mut pg = support::TestPostgres::start(17).await;
    let adapter = adapter_for(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/idle-in-transaction-sessions").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let sessions = body["data"].as_array().expect("data is an array");
    assert!(
        sessions.is_empty(),
        "expected no real idle-in-transaction session in a fresh fixture, got: {sessions:?}"
    );
}

/// Unit U53 (ADR 0046/0058): same connection config as `adapter_for`,
/// but with both activity thresholds overridden to `0.0` — any real
/// open transaction/idle-in-transaction session qualifies immediately,
/// with zero sleep/wait: by the time the HTTP handler's own query
/// runs, some real (however small) time has already elapsed since
/// `xact_start`/`state_change`, so `>= 0.0` is always true. No
/// flakiness risk, unlike waiting a fixed short delay.
fn adapter_for_with_zero_activity_thresholds(pg: &support::TestPostgres) -> Arc<dyn DbmsAdapter> {
    let cfg = service::dbms_config::resolve_db_connection_from(
        Some(pg.socket_dir.display().to_string()),
        Some(pg.port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("trust-auth-ignores-this".to_string()),
        Some("disable".to_string()),
        None,
        None,
    )
    .expect("host+password were both given");
    let pg_pool = pool::build_pool(&cfg.metadata, &cfg.password).expect("pool builds");
    Arc::new(PostgresAdapter::from_pool(pg_pool, false).with_activity_thresholds(0.0, 0.0))
}

/// Unit U53 (ADR 0046/0058): a real, genuinely open transaction —
/// `BEGIN;` with no further query, the same real fixture U49's
/// `sessions_returns_a_real_idle_in_transaction_backend_correctly`
/// already established — reported for real via the endpoint, closing
/// ADR 0046's own named populated-path gap.
#[tokio::test]
async fn long_transactions_reports_a_real_long_running_transaction() {
    let mut pg = support::TestPostgres::start(17).await;

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

    let adapter = adapter_for_with_zero_activity_thresholds(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/long-transactions").await;

    client
        .batch_execute("ROLLBACK;")
        .await
        .expect("release the real transaction");
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    let txns = body["data"].as_array().expect("data is an array");
    let txn = txns
        .iter()
        .find(|t| t["pid"] == backend_pid)
        .unwrap_or_else(|| {
            panic!("the real open transaction {backend_pid} must appear, got {body:?}")
        });
    assert!(
        txn["duration_seconds"]
            .as_f64()
            .expect("duration_seconds is a number")
            >= 0.0
    );
    assert_eq!(txn["state"], "IDLE_IN_TRANSACTION");
    assert_eq!(txn["username"], "postgres");
}

/// Unit U53 (ADR 0046/0058): closes the same named populated-path gap
/// for the sibling endpoint, using the identical real fixture.
#[tokio::test]
async fn idle_in_transaction_sessions_reports_a_real_idle_session() {
    let mut pg = support::TestPostgres::start(17).await;

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

    let adapter = adapter_for_with_zero_activity_thresholds(&pg);
    let app = build_app(Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/dbms/idle-in-transaction-sessions").await;

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
            panic!("the real idle-in-transaction session {backend_pid} must appear, got {body:?}")
        });
    assert!(
        session["idle_duration_seconds"]
            .as_f64()
            .expect("idle_duration_seconds is a number")
            >= 0.0
    );
    assert_eq!(session["username"], "postgres");
}
