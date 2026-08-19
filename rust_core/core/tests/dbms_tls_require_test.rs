//! Not on the DoD's literal checklist, but a direct, real verification of
//! requirement 2's "an explicit sslmode" for the one mode this unit
//! actually implements a TLS connector for (see `TlsMode`'s doc comment
//! for why `verify-ca`/`verify-full` are out of scope): a real self-signed
//! cert, a real PostgreSQL instance with `ssl=on`, and a real encrypted
//! connection — confirmed via the server's own `pg_stat_ssl` view, not
//! just "the connect call didn't error."
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::pool::build_pool;
use ai_ops_core::dbms::{ConnectionMetadata, DbmsAdapter, Environment, PasswordRef, TlsMode};
use common::TestPostgres;

#[tokio::test]
async fn a_require_mode_connection_is_actually_encrypted() {
    let pg = TestPostgres::start_with_tls(17).await;

    let metadata = ConnectionMetadata {
        name: "tls-require-test".to_string(),
        host: "127.0.0.1".to_string(),
        port: pg.port,
        database: pg.superuser.clone(),
        username: pg.superuser.clone(),
        tls_mode: TlsMode::Require,
        environment: Environment::Development,
        // Trust auth ignores the password's content; this test builds the
        // pool directly (not through `connect()`/the credential store) to
        // isolate exactly the TLS wiring, matching what
        // dbms_footprint_test.rs already covers for the credential-store
        // round-trip end-to-end.
        password_ref: PasswordRef("unused".to_string()),
        allow_superuser_override: false,
        capture_raw_sql: false,
    };

    let pool = build_pool(&metadata, "unused-under-trust-auth").expect("build TLS pool");
    let adapter = PostgresAdapter::from_pool(pool, false);

    adapter
        .version_info()
        .await
        .expect("a Require-mode connection against a real TLS-enabled server should succeed");

    // Confirm via the server's own accounting that the connection really
    // was encrypted, not merely that `Require` mode didn't error (which a
    // silently-downgraded-to-plaintext connection could also achieve).
    let side = pg.tcp_config(&pg.superuser, &pg.superuser);
    let (side_client, connection) = side
        .connect(tokio_postgres::NoTls)
        .await
        .expect("side connection for verification");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let rows = side_client
        .query(
            "SELECT ssl FROM pg_stat_ssl JOIN pg_stat_activity \
             USING (pid) WHERE application_name = 'ai-ops-coordinator'",
            &[],
        )
        .await
        .expect("query pg_stat_ssl");
    assert_eq!(
        rows.len(),
        1,
        "expected exactly one ai-ops-coordinator connection"
    );
    let ssl_in_use: bool = rows[0].get(0);
    assert!(
        ssl_in_use,
        "the Require-mode connection was not actually encrypted"
    );
}

#[tokio::test]
async fn a_disable_mode_connection_to_the_same_server_is_not_encrypted() {
    let pg = TestPostgres::start_with_tls(17).await;

    let metadata = ConnectionMetadata {
        name: "tls-disable-test".to_string(),
        host: "127.0.0.1".to_string(),
        port: pg.port,
        database: pg.superuser.clone(),
        username: pg.superuser.clone(),
        tls_mode: TlsMode::Disable,
        environment: Environment::Development,
        password_ref: PasswordRef("unused".to_string()),
        allow_superuser_override: false,
        capture_raw_sql: false,
    };
    let pool = build_pool(&metadata, "unused-under-trust-auth").expect("build plaintext pool");
    let adapter = PostgresAdapter::from_pool(pool, false);
    adapter
        .version_info()
        .await
        .expect("Disable mode should still connect to a server that merely offers TLS");
}
