//! Requirement 6: a missing `pg_stat_statements` degrades `query_stats()`
//! to `Unavailable` with a reason, and — the part that actually needs its
//! own dedicated test, beyond what the smoke test's fixture already shows
//! incidentally — every *other* method still works normally on the same
//! instance.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::{capability::Gated, DbmsAdapter};
use common::TestPostgres;

#[tokio::test]
async fn query_stats_degrades_without_breaking_everything_else() {
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

    match adapter.query_stats().await.expect("query_stats") {
        Gated::Unavailable { reason } => {
            assert!(
                reason.to_lowercase().contains("pg_stat_statements"),
                "reason should name the missing extension: {reason}"
            );
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }

    // Nothing else on this instance should be affected by the missing
    // extension — a handful of representative methods, not the full set
    // (already covered by dbms_adapter_smoke_test.rs).
    adapter
        .version_info()
        .await
        .expect("version_info still works");
    adapter
        .list_databases()
        .await
        .expect("list_databases still works");
    adapter
        .list_sessions()
        .await
        .expect("list_sessions still works");
    adapter
        .table_stats()
        .await
        .expect("table_stats still works");
}
