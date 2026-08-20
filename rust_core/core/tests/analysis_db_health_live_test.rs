//! One real-PostgreSQL integration test proving the `DbEvidenceBundle`
//! assembly glue actually compiles and works against real `DbmsAdapter`
//! output types — not just hand-built fixtures (see
//! analysis_db_health_test.rs for the exhaustive fixture coverage).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use ai_ops_core::analysis::{classify_db, DbEvidenceBundle, DbHealthCategory, DbHealthStatus};
use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::DbmsAdapter;
use chrono::Utc;
use common::TestPostgres;

#[tokio::test]
async fn classify_db_over_a_real_postgres_poll() {
    let pg = TestPostgres::start(17).await;

    let manager = deadpool_postgres::Manager::new(
        pg.config(&pg.superuser, &pg.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    let bundle = DbEvidenceBundle {
        databases: adapter.list_databases().await.expect("list_databases"),
        sessions: adapter.list_sessions().await.expect("list_sessions"),
        query_stats: adapter.query_stats().await.expect("query_stats"),
        locks: adapter.lock_graph().await.expect("lock_graph"),
        table_stats: adapter.table_stats().await.expect("table_stats"),
        replication: adapter
            .replication_status()
            .await
            .expect("replication_status"),
        gucs: adapter.relevant_gucs().await.expect("relevant_gucs"),
        temp_file_activity: adapter
            .temp_file_activity()
            .await
            .expect("temp_file_activity"),
        deadlocks: adapter.deadlock_history().await.expect("deadlock_history"),
        long_transactions: adapter
            .long_transactions()
            .await
            .expect("long_transactions"),
    };

    // Real data: the harness's own default database has commit activity
    // from setup/connection, so rollback ratio should be a sane real number.
    let commit_or_rollback_seen = bundle
        .databases
        .iter()
        .any(|d| d.xact_commit > 0 || d.xact_rollback > 0);
    assert!(
        commit_or_rollback_seen,
        "expected real xact_commit/xact_rollback activity from a live connection"
    );

    let verdict = classify_db(&bundle, Utc::now());

    assert!(!verdict.checks.is_empty());
    let availability = verdict
        .checks
        .iter()
        .find(|c| c.category == DbHealthCategory::Availability)
        .expect("availability check present");
    assert_eq!(availability.status, DbHealthStatus::Ok);
    // A quiet fresh instance under test should score cleanly.
    assert!(
        verdict.score > 50,
        "expected a healthy score, got {}",
        verdict.score
    );
}
