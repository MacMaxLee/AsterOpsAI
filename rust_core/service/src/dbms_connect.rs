//! Wires `core::dbms::role_check::validate` into `service`'s real DB
//! connection path — closing ADR 0025's own named gap. `dbms_config.rs`
//! builds a pool and (previously) handed it straight to `PostgresAdapter::
//! from_pool`, which — unlike `PostgresAdapter::connect` — never calls
//! `role_check::validate` on its own. This module runs that check
//! explicitly, once, against the same pool, before wrapping it for
//! `service`'s use.
//!
//! Lives in `service/src/`, not inline in `main.rs`, so `service/tests/
//! dbms_connect_test.rs` — an integration test against the `service`
//! *library* crate — can exercise it directly against a real `TestPostgres`
//! instance; `main.rs`'s own binary crate isn't reachable from there.

use std::sync::Arc;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::{pool, role_check, ConnectionMetadata, DbmsAdapter};
use ai_ops_core::repository::{self, RepositoryHandle};
use chrono::Utc;

/// Builds the pool, runs the least-privilege role check against it, and
/// (best-effort) records an audit event when a superuser override was
/// actually used. Every failure path — pool build, checkout, or a real
/// `PermissionDenied` rejection — logs and returns `None`, the same
/// resilience precedent `main.rs` already applied to a pool-build failure
/// before this unit: `service` keeps running without a DB adapter rather
/// than crashing over a DB connectivity/permission problem.
pub async fn connect_and_validate(
    metadata: &ConnectionMetadata,
    password: &str,
    repository: Option<&RepositoryHandle>,
) -> Option<Arc<dyn DbmsAdapter>> {
    let pg_pool = match pool::build_pool(metadata, password) {
        Ok(pg_pool) => pg_pool,
        Err(err) => {
            tracing::error!(error = %err, "could not build the configured DB connection pool; continuing without it");
            return None;
        }
    };

    let client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            tracing::error!(error = %err, "could not check out a connection from the configured DB pool; continuing without it");
            return None;
        }
    };

    let outcome = match role_check::validate(
        &client,
        metadata.environment,
        metadata.allow_superuser_override,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(err) => {
            tracing::error!(error = %err, "DB connection failed the least-privilege role check; continuing without it");
            return None;
        }
    };
    drop(client);

    if outcome.override_used {
        if let Some(handle) = repository {
            let event = role_check::audit_event_for_override(&metadata.name, Utc::now());
            if let Err(err) = repository::record_audit_event(handle, event).await {
                tracing::warn!(error = %err, "failed to record DBMS superuser-override audit event");
            }
        }
    }

    Some(Arc::new(PostgresAdapter::from_pool(
        pg_pool,
        metadata.capture_raw_sql,
    )))
}
