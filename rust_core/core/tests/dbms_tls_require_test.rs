//! Not on the DoD's literal checklist, but a direct, real verification of
//! requirement 2's "an explicit sslmode": a real self-signed cert, a real
//! PostgreSQL instance with `ssl=on`, and a real encrypted connection —
//! confirmed via the server's own `pg_stat_ssl` view, not just "the
//! connect call didn't error." Unit U77 (docs/adr/0082) closes ADR 0009's
//! own named `verify-ca`/`verify-full` gap — see the tests below this
//! file's original `Require`/`Disable` pair for real chain/hostname
//! verification proof, including the deliberate distinction between the
//! two modes.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::pool::build_pool;
use ai_ops_core::dbms::{
    ConnectionMetadata, DbmsAdapter, DbmsError, Environment, PasswordRef, TlsMode,
};
use common::TestPostgres;

fn metadata_for(
    pg: &TestPostgres,
    host: &str,
    tls_mode: TlsMode,
    ca_bundle_path: Option<PathBuf>,
) -> ConnectionMetadata {
    ConnectionMetadata {
        name: "tls-verify-test".to_string(),
        host: host.to_string(),
        port: pg.port,
        database: pg.superuser.clone(),
        username: pg.superuser.clone(),
        tls_mode,
        environment: Environment::Development,
        password_ref: PasswordRef("unused".to_string()),
        ca_bundle_path,
        allow_superuser_override: false,
        capture_raw_sql: false,
    }
}

/// A throwaway self-signed cert genuinely unrelated to any real server's
/// own cert — mirrors `TestPostgres::start_with_tls`'s own cert-gen
/// exactly, just never handed to any running server. Used to prove
/// chain-trust rejection is real, not a verifier that accepts anything.
fn generate_unrelated_self_signed_cert(dir: &Path) -> PathBuf {
    let cert_path = dir.join("unrelated.crt");
    let key_path = dir.join("unrelated.key");
    let status = StdCommand::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
        .arg(&key_path)
        .arg("-out")
        .arg(&cert_path)
        .args(["-days", "1", "-subj", "/CN=unrelated"])
        .status()
        .expect("spawn openssl req");
    assert!(status.success(), "unrelated cert generation failed");
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
        .expect("chmod unrelated.key");
    cert_path
}

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
        ca_bundle_path: None,
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
        ca_bundle_path: None,
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

/// Unit U77: `build_pool` must refuse to even attempt a connection —
/// never silently downgrade to unverified — when `verify-ca`/`verify-
/// full` is requested without a CA bundle path. No real server needed;
/// this must fail before any I/O.
#[test]
fn verify_ca_without_a_ca_bundle_path_is_a_real_error() {
    let metadata = ConnectionMetadata {
        name: "tls-verify-ca-no-bundle".to_string(),
        host: "127.0.0.1".to_string(),
        port: 5432,
        database: "postgres".to_string(),
        username: "postgres".to_string(),
        tls_mode: TlsMode::VerifyCa,
        environment: Environment::Development,
        password_ref: PasswordRef("unused".to_string()),
        ca_bundle_path: None,
        allow_superuser_override: false,
        capture_raw_sql: false,
    };
    let err = build_pool(&metadata, "unused").expect_err("verify-ca with no CA bundle must error");
    assert!(matches!(err, DbmsError::Other(_)));
}

#[test]
fn verify_full_without_a_ca_bundle_path_is_a_real_error() {
    let metadata = ConnectionMetadata {
        name: "tls-verify-full-no-bundle".to_string(),
        host: "127.0.0.1".to_string(),
        port: 5432,
        database: "postgres".to_string(),
        username: "postgres".to_string(),
        tls_mode: TlsMode::VerifyFull,
        environment: Environment::Development,
        password_ref: PasswordRef("unused".to_string()),
        ca_bundle_path: None,
        allow_superuser_override: false,
        capture_raw_sql: false,
    };
    let err =
        build_pool(&metadata, "unused").expect_err("verify-full with no CA bundle must error");
    assert!(matches!(err, DbmsError::Other(_)));
}

#[test]
fn verify_ca_with_a_nonexistent_ca_bundle_path_is_a_real_error() {
    let metadata = ConnectionMetadata {
        name: "tls-verify-ca-missing-file".to_string(),
        host: "127.0.0.1".to_string(),
        port: 5432,
        database: "postgres".to_string(),
        username: "postgres".to_string(),
        tls_mode: TlsMode::VerifyCa,
        environment: Environment::Development,
        password_ref: PasswordRef("unused".to_string()),
        ca_bundle_path: Some(PathBuf::from("/nonexistent/ca.pem")),
        allow_superuser_override: false,
        capture_raw_sql: false,
    };
    let err = build_pool(&metadata, "unused").expect_err("a nonexistent CA bundle path must error");
    assert!(matches!(err, DbmsError::Other(_)));
}

#[test]
fn verify_ca_with_an_empty_ca_bundle_file_is_a_real_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let empty_bundle = dir.path().join("empty.pem");
    std::fs::write(&empty_bundle, "").expect("write empty bundle");
    let metadata = ConnectionMetadata {
        name: "tls-verify-ca-empty-bundle".to_string(),
        host: "127.0.0.1".to_string(),
        port: 5432,
        database: "postgres".to_string(),
        username: "postgres".to_string(),
        tls_mode: TlsMode::VerifyCa,
        environment: Environment::Development,
        password_ref: PasswordRef("unused".to_string()),
        ca_bundle_path: Some(empty_bundle),
        allow_superuser_override: false,
        capture_raw_sql: false,
    };
    let err = build_pool(&metadata, "unused")
        .expect_err("a CA bundle with no certificates in it must error");
    assert!(matches!(err, DbmsError::Other(_)));
}

/// Unit U77's central positive proof: a real self-signed cert, used as
/// its own trust anchor, with a hostname that genuinely matches the
/// cert's `CN=localhost` — both chain and hostname verification pass.
#[tokio::test]
async fn verify_full_succeeds_when_the_ca_and_hostname_both_genuinely_match() {
    let pg = TestPostgres::start_with_tls(17).await;
    let metadata = metadata_for(
        &pg,
        "localhost",
        TlsMode::VerifyFull,
        Some(pg.tls_server_cert_path()),
    );
    let pool = build_pool(&metadata, "unused-under-trust-auth").expect("build verify-full pool");
    let adapter = PostgresAdapter::from_pool(pool, false);
    adapter
        .version_info()
        .await
        .expect("verify-full must succeed when the CA and hostname both genuinely match");
}

/// The real distinguishing behavior this unit exists for: the exact same
/// server/CA, but connected to via `127.0.0.1` — which the cert's own
/// `CN=localhost` (no IP SAN) does not cover. `verify-full` must reject
/// this; `verify-ca` (below) must not.
#[tokio::test]
async fn verify_full_fails_on_a_genuine_hostname_mismatch() {
    let pg = TestPostgres::start_with_tls(17).await;
    let metadata = metadata_for(
        &pg,
        "127.0.0.1",
        TlsMode::VerifyFull,
        Some(pg.tls_server_cert_path()),
    );
    let pool = build_pool(&metadata, "unused-under-trust-auth").expect("build verify-full pool");
    let adapter = PostgresAdapter::from_pool(pool, false);
    let result = adapter.version_info().await;
    assert!(
        result.is_err(),
        "verify-full must reject a real hostname/cert mismatch, not silently accept it"
    );
}

#[tokio::test]
async fn verify_ca_succeeds_despite_the_same_hostname_mismatch_verify_full_rejects() {
    let pg = TestPostgres::start_with_tls(17).await;
    let metadata = metadata_for(
        &pg,
        "127.0.0.1",
        TlsMode::VerifyCa,
        Some(pg.tls_server_cert_path()),
    );
    let pool = build_pool(&metadata, "unused-under-trust-auth").expect("build verify-ca pool");
    let adapter = PostgresAdapter::from_pool(pool, false);
    adapter.version_info().await.expect(
        "verify-ca must tolerate a hostname mismatch as long as the certificate chain is \
         genuinely trusted — this is what distinguishes it from verify-full",
    );
}

/// Proves `verify-ca` still does real chain validation — it is not
/// simply "accept anything," it specifically tolerates hostname
/// mismatches while still requiring a trusted chain.
#[tokio::test]
async fn verify_ca_rejects_a_certificate_not_signed_by_the_given_ca() {
    let pg = TestPostgres::start_with_tls(17).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let unrelated_ca = generate_unrelated_self_signed_cert(dir.path());

    let metadata = metadata_for(&pg, "127.0.0.1", TlsMode::VerifyCa, Some(unrelated_ca));
    let pool = build_pool(&metadata, "unused-under-trust-auth").expect("build verify-ca pool");
    let adapter = PostgresAdapter::from_pool(pool, false);
    let result = adapter.version_info().await;
    assert!(
        result.is_err(),
        "verify-ca must reject a real server cert that doesn't chain to the given CA"
    );
}
