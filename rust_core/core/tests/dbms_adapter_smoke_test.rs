//! Full `DbmsAdapter` method set, exercised against real PostgreSQL 13, 15,
//! and 17 server instances (the DoD's version matrix) — one shared body,
//! run once per version so a failure clearly names which version broke.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::{capability::Gated, DbmsAdapter};
use common::TestPostgres;

async fn setup_fixture(pg: &TestPostgres) {
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

async fn run_full_method_set(major_version: u32) {
    let pg = TestPostgres::start(major_version).await;
    setup_fixture(&pg).await;

    let manager = deadpool_postgres::Manager::new(
        pg.config(&pg.superuser, &pg.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    let version = adapter.version_info().await.expect("version_info");
    assert!(version.server_version_num >= major_version as i32 * 10000);
    assert!(version.version_string.contains("PostgreSQL"));

    let databases = adapter.list_databases().await.expect("list_databases");
    assert!(databases.iter().any(|d| d.name == pg.superuser));

    let sessions = adapter.list_sessions().await.expect("list_sessions");
    assert!(!sessions.is_empty(), "expected at least our own session");

    // query_stats: pg_stat_statements isn't loaded via shared_preload_libraries
    // in this fixture, so this instance genuinely lacks it — exercises the
    // same degrade-to-Unavailable path as the dedicated no-pg_stat_statements
    // test, from a different angle (a real, not specially-crafted, instance).
    match adapter.query_stats().await.expect("query_stats") {
        Gated::Unavailable { .. } => {}
        other => panic!("expected Unavailable without shared_preload_libraries, got {other:?}"),
    }

    let locks = adapter.lock_graph().await.expect("lock_graph");
    assert!(locks.is_empty(), "no contention expected in this fixture");

    let tables = adapter.table_stats().await.expect("table_stats");
    assert!(tables.iter().any(|t| t.table == "widgets"));

    let indexes = adapter.index_stats().await.expect("index_stats");
    assert!(indexes.iter().any(|i| i.index == "idx_widgets_name"));

    let replication = adapter
        .replication_status()
        .await
        .expect("replication_status");
    assert!(replication.is_primary);
    assert!(!replication.in_recovery);
    assert!(replication.standbys.is_empty());

    let gucs = adapter.relevant_gucs().await.expect("relevant_gucs");
    assert!(gucs.iter().any(|g| g.name == "max_connections"));

    let temp = adapter
        .temp_file_activity()
        .await
        .expect("temp_file_activity");
    assert!(temp.temp_files >= 0);

    let deadlocks = adapter.deadlock_history().await.expect("deadlock_history");
    assert_eq!(deadlocks.deadlocks, 0);

    let long_txns = adapter
        .long_transactions()
        .await
        .expect("long_transactions");
    assert!(
        long_txns.is_empty(),
        "no long-running transaction in this fixture"
    );

    let idle_txns = adapter
        .idle_in_transaction_sessions()
        .await
        .expect("idle_in_transaction_sessions");
    assert!(idle_txns.is_empty());
}

#[tokio::test]
async fn pg13_full_method_set() {
    run_full_method_set(13).await;
}

#[tokio::test]
async fn pg15_full_method_set() {
    run_full_method_set(15).await;
}

#[tokio::test]
async fn pg17_full_method_set() {
    run_full_method_set(17).await;
}
