//! Real end-to-end coverage of the cross-layer correlation endpoint
//! (unit U20): a real axum router, a real SQLite-backed `RepositoryHandle`
//! for the host side, and a real, throwaway PostgreSQL instance
//! (`support::TestPostgres`) with genuinely induced lock contention for
//! the DB side — proving `classify_db` + `correlate()` themselves ran
//! against real data, not a hand-built fixture response.
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
use ai_ops_core::repository::{self, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use service::{api, self_metrics, state::AppState, telemetry};
use tower::ServiceExt;

async fn build_app(
    repository: Option<repository::RepositoryHandle>,
    dbms_adapter: Option<Arc<dyn DbmsAdapter>>,
) -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(
        platform,
        self_metrics,
        host_telemetry,
        repository,
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

#[tokio::test]
async fn correlation_is_unavailable_without_a_repository() {
    let app = build_app(None, None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn no_db_configured_rules_out_every_db_side_cause_honestly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite3");
    let repo = repository::init(&RepositoryConfig { db_path }).expect("repository inits");
    let app = build_app(Some(repo), None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation").await;
    assert_eq!(status, StatusCode::OK);

    let ruled_out = body["data"]["ruled_out"].as_array().unwrap();
    let db_causes = [
        "DB_LOCKS",
        "DB_CONFIGURATION",
        "CONNECTION_EXHAUSTION",
        "SLOW_SQL",
    ];
    for cause in db_causes {
        let entry = ruled_out
            .iter()
            .find(|r| r["cause"] == cause)
            .unwrap_or_else(|| panic!("{cause} should be ruled out with no DB configured"));
        assert_eq!(entry["reason"], "no DB evidence available");
    }
    let ranked = body["data"]["ranked"].as_array().unwrap();
    assert!(
        !ranked
            .iter()
            .any(|h| db_causes.contains(&h["cause"].as_str().unwrap())),
        "no DB cause should ever be ranked with no DB configured, got {ranked:?}"
    );
}

#[tokio::test]
async fn a_refused_db_connection_degrades_to_ruled_out_never_a_503() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite3");
    let repo = repository::init(&RepositoryConfig { db_path }).expect("repository inits");
    // Bind then immediately drop, so nothing is listening on this port —
    // a real, deterministic "connection refused", not a guessed one.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);

    let cfg = service::dbms_config::resolve_db_connection_from(
        Some("127.0.0.1".to_string()),
        Some(port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("irrelevant".to_string()),
        Some("disable".to_string()),
        None,
        None,
        None,
    )
    .expect("host+password were both given");
    let pg_pool = pool::build_pool(&cfg.metadata, &cfg.password).expect("pool builds lazily");
    let adapter: Arc<dyn DbmsAdapter> = Arc::new(PostgresAdapter::from_pool(pg_pool, false));

    let app = build_app(Some(repo), Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation").await;
    assert_eq!(status, StatusCode::OK);
    let ruled_out = body["data"]["ruled_out"].as_array().unwrap();
    assert!(
        ruled_out.iter().any(|r| r["cause"] == "DB_LOCKS"),
        "a refused connection must still produce a real (ruled-out) verdict, got {body:?}"
    );
}

#[tokio::test]
async fn a_real_lock_storm_ranks_db_locks_from_genuinely_induced_contention() {
    let mut pg = support::TestPostgres::start(17).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite3");
    let repo = repository::init(&RepositoryConfig { db_path }).expect("repository inits");

    let setup = pg.superuser_client().await;
    setup
        .batch_execute("CREATE TABLE t (id int); INSERT INTO t VALUES (1)")
        .await
        .expect("create table and seed a row");

    // The lock holder: an open transaction holding a real row-level lock
    // via `SELECT ... FOR UPDATE`, never committed until this test
    // releases it. Deliberately row-level, not `LOCK TABLE ... ACCESS
    // EXCLUSIVE MODE` — a real discovery while writing this test: a
    // whole-table exclusive lock also blocks the adapter's own
    // `table_stats` poll (`pg_total_relation_size` needs an
    // AccessShareLock on the table), which starves the endpoint's own
    // `statement_timeout` and degrades the whole bundle to
    // `unavailable_verdict` instead of demonstrating the lock storm. A
    // row-level lock is compatible with AccessShareLock, so the
    // endpoint's own poll is unaffected — a real, honest reproduction of
    // an ordinary row-contention lock storm, not a self-inflicted outage.
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

    // Six real waiters, each genuinely blocked on the holder's row lock —
    // one more than DB_LOCK_WAIT_COUNT_HIGH (5.0), so the lock-waits
    // domain crosses into a real, non-Ok tier.
    for _ in 0..6 {
        let (waiter, waiter_conn) = pg
            .config("postgres", "postgres")
            .connect(tokio_postgres::NoTls)
            .await
            .expect("connect waiter");
        tokio::spawn(async move {
            let _ = waiter_conn.await;
        });
        tokio::spawn(async move {
            let _ = waiter
                .simple_query("SELECT id FROM t WHERE id = 1 FOR UPDATE")
                .await;
        });
    }

    // Poll pg_stat_activity for the real block to register — no guessed
    // sleep, the same discipline this session used for SIGSTOP/SIGCONT.
    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(10);
    loop {
        let row = setup
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type = 'Lock'",
                &[],
            )
            .await
            .expect("query pg_stat_activity");
        let waiting: i64 = row.get(0);
        if waiting >= 6 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the six waiters never registered as blocked in pg_stat_activity"
        );
        tokio::time::sleep(StdDuration::from_millis(50)).await;
    }

    let cfg = service::dbms_config::resolve_db_connection_from(
        Some(pg.socket_dir.display().to_string()),
        Some(pg.port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("trust-auth-ignores-this".to_string()),
        Some("disable".to_string()),
        None,
        None,
        None,
    )
    .expect("host+password were both given");
    let pg_pool = pool::build_pool(&cfg.metadata, &cfg.password).expect("pool builds");
    let adapter: Arc<dyn DbmsAdapter> = Arc::new(PostgresAdapter::from_pool(pg_pool, false));

    let app = build_app(Some(repo), Some(adapter)).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation").await;

    holder
        .batch_execute("ROLLBACK")
        .await
        .expect("release the row lock");
    pg.stop().await;

    assert_eq!(status, StatusCode::OK);
    let ranked = body["data"]["ranked"].as_array().unwrap();
    let db_locks = ranked
        .iter()
        .find(|h| h["cause"] == "DB_LOCKS")
        .unwrap_or_else(|| {
            panic!("DB_LOCKS must be ranked from six real blocked sessions, got {body:?}")
        });
    let evidence = db_locks["evidence"].as_array().unwrap();
    assert!(
        evidence
            .iter()
            .any(|e| e["metric"] == "blocking_lock_edges"),
        "DB_LOCKS' evidence should include the real blocking_lock_edges metric, got {evidence:?}"
    );
}
