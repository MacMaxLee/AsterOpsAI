//! The DoD's mid-poll-disconnect case: kill the real PostgreSQL server
//! process while the adapter is mid-collection, confirm a clean error (not
//! a panic/hang), then confirm the pool serves real requests again once
//! PostgreSQL is restarted — deadpool-postgres's own reconnect behavior,
//! verified live against a real killed-and-restarted server, not assumed
//! from its documentation.
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
async fn the_pool_recovers_after_the_server_is_killed_and_restarted() {
    let mut pg = TestPostgres::start(17).await;

    let manager = deadpool_postgres::Manager::new(
        pg.config(&pg.superuser, &pg.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    adapter
        .version_info()
        .await
        .expect("a healthy server answers before being killed");

    // A hard, immediate stop — not a graceful shutdown — to actually
    // simulate an unexpected mid-session disconnect, not a cooperative one.
    pg.stop().await;

    // Give in-flight/pooled connections a moment to actually notice the
    // socket is gone rather than racing the OS's own TCP/socket teardown.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = tokio::time::timeout(Duration::from_secs(10), adapter.version_info()).await;
    match result {
        Ok(Err(_)) => {} // a clean DbmsError — correct
        Ok(Ok(_)) => panic!("expected an error against a killed server, got Ok"),
        Err(_) => panic!("adapter call hung instead of returning a clean error"),
    }

    // Restart the *same* instance (same data dir, same port/socket) and
    // confirm the same pool — not a freshly constructed one — recovers on
    // its own.
    pg.start_again().await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let recovered = tokio::time::timeout(Duration::from_secs(10), adapter.version_info()).await;
    match recovered {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => panic!("pool did not recover after the server restarted: {e}"),
        Err(_) => panic!("adapter call hung waiting for recovery"),
    }
}
