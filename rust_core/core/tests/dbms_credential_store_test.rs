//! TRS §18 / SRS NFR-PRIV-002: round-trips a real secret through the real
//! OS credential store. `org.freedesktop.secrets` (gnome-keyring) is
//! confirmed live on this session's D-Bus — verified directly, not
//! assumed — but a CI runner or another dev machine may have no Secret
//! Service provider at all (headless environments commonly don't), so
//! every test here skips itself with a visible, explicit message rather
//! than failing when `KeyringCredentialStore` reports `NoStore` — that's
//! an environment fact, not a bug in this code.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::dbms::{
    CredentialStore, CredentialStoreError, KeyringCredentialStore, PasswordRef,
};

#[test]
fn stores_fetches_and_deletes_a_real_secret() {
    let store = KeyringCredentialStore::new();
    let key = PasswordRef("dbms-credential-store-test-roundtrip".to_string());

    match store.store(&key, "correct-horse-battery-staple") {
        Ok(()) => {}
        Err(CredentialStoreError::NoStore(reason)) => {
            eprintln!("SKIPPED: no OS credential store available in this environment: {reason}");
            return;
        }
        Err(other) => panic!("unexpected error storing a credential: {other}"),
    }

    let fetched = store.fetch(&key).expect("fetch");
    assert_eq!(fetched, "correct-horse-battery-staple");

    store.delete(&key).expect("delete");
    match store.fetch(&key) {
        Err(CredentialStoreError::NotFound) => {}
        other => panic!("expected NotFound after delete, got {other:?}"),
    }
}

#[test]
fn fetching_a_never_stored_key_is_not_found_not_a_panic() {
    let store = KeyringCredentialStore::new();
    let key = PasswordRef("dbms-credential-store-test-never-stored".to_string());

    match store.fetch(&key) {
        Err(CredentialStoreError::NotFound) => {}
        Err(CredentialStoreError::NoStore(reason)) => {
            eprintln!("SKIPPED: no OS credential store available in this environment: {reason}");
        }
        other => panic!("expected NotFound, got {other:?}"),
    }
}
