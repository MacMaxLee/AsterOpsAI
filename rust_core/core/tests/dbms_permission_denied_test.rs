//! Requirement 4 (least privilege) and the DoD's "permission-denied case":
//! two real, distinct permission scenarios — a superuser connection
//! refused against a `Production`-classified instance, and a real SQL
//! permission-denied error (from an actual `REVOKE`, not simulated)
//! surfacing as a clean typed error rather than a panic.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::role_check::validate;
use ai_ops_core::dbms::{DbmsAdapter, DbmsError, Environment};
use common::TestPostgres;

#[tokio::test]
async fn a_superuser_connection_is_refused_in_production_without_an_override() {
    let pg = TestPostgres::start(17).await;
    let client = pg.superuser_client().await; // initdb's -U role is a real superuser

    let result = validate(&client, Environment::Production, false).await;
    match result {
        Err(DbmsError::PermissionDenied(reason)) => {
            assert!(reason.contains("superuser"));
        }
        other => panic!("expected PermissionDenied, got {other:?}"),
    }
}

#[tokio::test]
async fn a_superuser_connection_is_allowed_in_production_with_an_explicit_override() {
    let pg = TestPostgres::start(17).await;
    let client = pg.superuser_client().await;

    let outcome = validate(&client, Environment::Production, true)
        .await
        .expect("override should let it through");
    assert!(outcome.rolsuper);
    assert!(outcome.override_used);
}

#[tokio::test]
async fn superuser_in_non_production_environments_needs_no_override() {
    let pg = TestPostgres::start(17).await;
    let client = pg.superuser_client().await;

    for env in [Environment::Development, Environment::Staging] {
        let outcome = validate(&client, env, false)
            .await
            .expect("non-production superuser connections aren't refused");
        assert!(!outcome.override_used);
    }
}

#[tokio::test]
async fn a_real_sql_permission_error_surfaces_cleanly_not_as_a_panic() {
    let pg = TestPostgres::start(17).await;
    let admin = pg.superuser_client().await;

    // A real, minimal role with no monitoring grants at all, and an
    // explicit REVOKE of the default PUBLIC SELECT grant on a catalog view
    // this adapter reads — a genuine PostgreSQL permission-denied error,
    // not a simulated one.
    admin
        .batch_execute(
            "CREATE ROLE dbms_test_no_privs LOGIN; \
             REVOKE SELECT ON pg_stat_activity FROM PUBLIC;",
        )
        .await
        .expect("create restricted role");

    let manager = deadpool_postgres::Manager::new(
        pg.config("dbms_test_no_privs", &pg.superuser),
        tokio_postgres::NoTls,
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .expect("build pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    match adapter.list_sessions().await {
        Err(DbmsError::Query(_)) => {}
        other => panic!("expected a clean Query error, got {other:?}"),
    }
}
