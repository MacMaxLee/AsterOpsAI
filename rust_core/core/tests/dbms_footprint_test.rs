//! FR-DB-002 / the DoD's literal footprint requirement: confirmed via a
//! side connection's own query against the real `pg_stat_activity` (not
//! asserted from pool config alone) that the tool's footprint against the
//! monitored instance is exactly one idle connection between polls.
//!
//! Also exercises the real `PostgresAdapter::connect()` path end-to-end —
//! including a real credential-store round-trip through the real Secret
//! Service running in this session (see `dbms_credential_store_test.rs`
//! for the dedicated, skippable-if-unavailable version of that check) —
//! rather than the `from_pool` test shortcut every other dbms test uses,
//! since this is the one test that specifically needs `application_name`
//! to have been set via the real production connection path.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use std::time::Duration;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::{
    ConnectionMetadata, CredentialStore, CredentialStoreError, DbmsAdapter, Environment,
    KeyringCredentialStore, PasswordRef, TlsMode,
};
use common::TestPostgres;

#[tokio::test]
async fn exactly_one_idle_connection_between_polls() {
    let pg = TestPostgres::start(17).await;

    let credential_store = KeyringCredentialStore::new();
    let password_ref = PasswordRef(format!("dbms-footprint-test-{}", pg.port));
    // Trust auth on this fixture ignores the password's actual content —
    // this exercises the real store/fetch round-trip, not the value.
    // Skips itself (mirrors dbms_credential_store_test.rs) rather than
    // failing when no OS credential store is available — a headless CI
    // runner commonly has no Secret Service provider, which is an
    // environment fact, not a bug in this code.
    match credential_store.store(&password_ref, "unused-under-trust-auth") {
        Ok(()) => {}
        Err(CredentialStoreError::NoStore(reason)) => {
            eprintln!("SKIPPED: no OS credential store available in this environment: {reason}");
            return;
        }
        Err(other) => panic!("unexpected error storing a credential: {other}"),
    }

    let metadata = ConnectionMetadata {
        name: "footprint-test".to_string(),
        host: pg.socket_dir.to_string_lossy().into_owned(),
        port: pg.port,
        database: pg.superuser.clone(),
        username: pg.superuser.clone(),
        tls_mode: TlsMode::Disable,
        environment: Environment::Development,
        password_ref: password_ref.clone(),
        ca_bundle_path: None,
        allow_superuser_override: false,
        capture_raw_sql: false,
    };

    let adapter = PostgresAdapter::connect(&metadata, &credential_store)
        .await
        .expect("connect via the real production path");

    for _ in 0..3 {
        adapter.version_info().await.expect("poll");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Let the pool's connection settle back to idle after the last poll.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let side = pg.superuser_client().await;
    let rows = side
        .query(
            "SELECT state FROM pg_stat_activity WHERE application_name = 'ai-ops-coordinator'",
            &[],
        )
        .await
        .expect("side query against pg_stat_activity");

    assert_eq!(
        rows.len(),
        1,
        "expected exactly one ai-ops-coordinator connection, found {}",
        rows.len()
    );
    let state: String = rows[0].get(0);
    assert_eq!(
        state, "idle",
        "the one connection should be idle between polls"
    );

    let _ = credential_store.delete(&password_ref);
}
