//! SRS FR-DB-006 (unit U64): `lock_graph` against real lock contention —
//! `dbms_adapter_smoke_test.rs`'s own use of this method only proves the
//! empty/no-contention case, and nothing else in this suite ever creates
//! two real, concurrently blocking sessions. Two independent
//! `superuser_client()` connections (real backends, real PIDs, matching
//! `TestPostgres`'s own multi-session precedent in `dbms_footprint_test.rs`
//! and the replica tests) — one holds `ACCESS EXCLUSIVE` inside an open
//! transaction, the other genuinely blocks trying to acquire the same
//! lock — proves `lock_graph` reports the real blocking session, not just
//! the blocked one.
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
async fn lock_graph_reports_the_real_blocking_session_not_just_the_blocked_one() {
    let pg = TestPostgres::start(17).await;

    let setup = pg.superuser_client().await;
    setup
        .batch_execute("CREATE TABLE widgets (id serial PRIMARY KEY, name text);")
        .await
        .expect("create table");

    let blocking = pg.superuser_client().await;
    let blocking_pid: i32 = blocking
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("blocking pid")
        .get(0);
    blocking
        .batch_execute("BEGIN; LOCK TABLE widgets IN ACCESS EXCLUSIVE MODE;")
        .await
        .expect("acquire exclusive lock");

    let blocked = pg.superuser_client().await;
    let blocked_pid: i32 = blocked
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("blocked pid")
        .get(0);
    let blocked_task = tokio::spawn(async move {
        // `LOCK TABLE` outside an explicit transaction block is a real
        // Postgres error (`LOCK TABLE can only be used in transaction
        // blocks`), not an implicit auto-commit — needs the same `BEGIN`
        // wrapping the blocking session uses above, or this never blocks
        // at all, it just errors out immediately.
        blocked
            .batch_execute("BEGIN; LOCK TABLE widgets IN ACCESS EXCLUSIVE MODE;")
            .await
    });

    let manager = deadpool_postgres::Manager::new(
        pg.config(&pg.superuser, &pg.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    let mut edges = Vec::new();
    for _ in 0..50 {
        edges = adapter.lock_graph().await.expect("lock_graph");
        if !edges.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(edges.len(), 1, "expected exactly one blocking edge");
    let edge = &edges[0];
    assert_eq!(edge.blocked_pid, blocked_pid);
    assert_eq!(edge.blocking_pid, blocking_pid);
    assert!(!edge.lock_type.is_empty());
    assert!(edge.blocked_query.as_deref().is_some_and(|q| !q.is_empty()));
    assert!(edge
        .blocking_query
        .as_deref()
        .is_some_and(|q| !q.is_empty()));

    blocking
        .batch_execute("ROLLBACK;")
        .await
        .expect("release the lock");
    blocked_task
        .await
        .expect("blocked task join")
        .expect("blocked statement eventually succeeds");
}
