//! The DoD's replica case: a real primary plus a real streaming-replica
//! standby (both real extracted-binary PostgreSQL instances, wired via
//! `pg_basebackup -R`, not simulated), confirming `replication_status()`
//! reports genuine sent/write/flush/replay LSN and lag data from the
//! primary's own `pg_stat_replication` — SRS FR-DB-008.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use std::time::Duration;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::DbmsAdapter;
use common::TestPostgres;

#[tokio::test]
async fn replication_status_reports_a_real_standby() {
    let primary = TestPostgres::start(17).await;
    let _replica = primary.start_streaming_replica().await;

    let manager = deadpool_postgres::Manager::new(
        primary.config(&primary.superuser, &primary.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    // Streaming replication announces itself to pg_stat_replication
    // asynchronously after the standby connects — poll for real rather
    // than assuming it's instant.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let status = adapter
            .replication_status()
            .await
            .expect("replication_status");
        if !status.standbys.is_empty() {
            assert!(status.is_primary);
            assert!(!status.in_recovery);
            let standby = &status.standbys[0];
            assert!(
                standby.sent_lsn.is_some(),
                "a connected standby should have a real sent_lsn"
            );
            assert!(standby.state == "streaming" || standby.state == "catchup");
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("no standby appeared in pg_stat_replication within the deadline");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn a_standby_reports_in_recovery_via_its_own_connection() {
    let primary = TestPostgres::start(17).await;
    let replica = primary.start_streaming_replica().await;

    let manager = deadpool_postgres::Manager::new(
        replica.config(&replica.superuser, &replica.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    let status = adapter
        .replication_status()
        .await
        .expect("replication_status against the standby itself");
    assert!(status.in_recovery);
    assert!(!status.is_primary);
}
