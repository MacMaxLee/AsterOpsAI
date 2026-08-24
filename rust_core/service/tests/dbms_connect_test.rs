//! Real end-to-end coverage of unit U48: `dbms_connect::connect_and_validate`
//! actually calls `core::dbms::role_check::validate` against the pool
//! `dbms_config` builds — closing ADR 0025's own named gap (`PostgresAdapter::
//! from_pool`, what `service` actually uses, skips the least-privilege check
//! `PostgresAdapter::connect` performs automatically).
//!
//! `support::TestPostgres`'s own default role (`pg.superuser`, `"postgres"`)
//! is a real PostgreSQL superuser — classifying its connection metadata as
//! `Environment::Production` makes a genuinely real (not simulated) proof of
//! both the refusal and the override paths possible.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use ai_ops_core::repository::{self, RepositoryConfig};
use service::{dbms_config, dbms_connect};

fn cfg(
    pg: &support::TestPostgres,
    environment: Option<&str>,
    allow_superuser_override: Option<&str>,
) -> dbms_config::DbConnectionConfig {
    dbms_config::resolve_db_connection_from(
        Some(pg.socket_dir.display().to_string()),
        Some(pg.port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("trust-auth-ignores-this".to_string()),
        Some("disable".to_string()),
        environment.map(|s| s.to_string()),
        allow_superuser_override.map(|s| s.to_string()),
        None,
    )
    .expect("host+password were both given")
}

async fn open_repo() -> repository::RepositoryHandle {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.sqlite3");
    std::mem::forget(dir);
    repository::init(&RepositoryConfig { db_path }).expect("repository::init")
}

#[tokio::test]
async fn a_production_classified_superuser_connection_is_refused_without_an_override() {
    let mut pg = support::TestPostgres::start(17).await;
    let cfg = cfg(&pg, Some("production"), None);

    let adapter = dbms_connect::connect_and_validate(&cfg.metadata, &cfg.password, None).await;

    assert!(
        adapter.is_none(),
        "a real superuser role against a Production-classified instance must be refused"
    );

    pg.stop().await;
}

#[tokio::test]
async fn an_explicit_override_lets_the_connection_through_and_records_a_real_audit_event() {
    let mut pg = support::TestPostgres::start(17).await;
    let cfg = cfg(&pg, Some("production"), Some("true"));
    let repo = open_repo().await;

    let adapter =
        dbms_connect::connect_and_validate(&cfg.metadata, &cfg.password, Some(&repo)).await;

    assert!(
        adapter.is_some(),
        "an explicit override must let a superuser connection through"
    );
    let event_type = repository::latest_audit_event_type(&repo)
        .await
        .expect("audit read succeeds");
    assert_eq!(event_type, Some("DBMS_SUPERUSER_OVERRIDE".to_string()));

    pg.stop().await;
}

#[tokio::test]
async fn a_development_classified_connection_passes_through_with_no_audit_event() {
    let mut pg = support::TestPostgres::start(17).await;
    let cfg = cfg(&pg, None, None);
    let repo = open_repo().await;

    let adapter =
        dbms_connect::connect_and_validate(&cfg.metadata, &cfg.password, Some(&repo)).await;

    assert!(
        adapter.is_some(),
        "today's real default (Development, no override) must keep working unchanged"
    );
    let event_type = repository::latest_audit_event_type(&repo)
        .await
        .expect("audit read succeeds");
    assert_eq!(event_type, None);

    pg.stop().await;
}
