//! The only place a PostgreSQL connection's password ever exists as a
//! plain `String` in this process, and even then only transiently (fetched
//! at connect time, handed straight to the driver, never stored back into
//! a struct field — see `pool.rs`). Backed by the real OS credential store
//! via the `keyring` crate: Secret Service on Linux, Keychain on macOS,
//! Credential Manager on Windows (TRS §18, SRS NFR-PRIV-002).

use super::connection_metadata::PasswordRef;

/// The `keyring::Entry` "service" namespace every credential this tool
/// stores lives under, so it's identifiable (and revocable in bulk) in the
/// OS credential store's own UI, distinct from any other application's
/// entries.
const SERVICE: &str = "ai-ops-coordinator";

#[derive(Debug, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("no credential store is available on this platform: {0}")]
    NoStore(String),
    #[error("no credential found for this connection")]
    NotFound,
    #[error("credential store error: {0}")]
    Other(String),
}

impl From<keyring::Error> for CredentialStoreError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoDefaultStore => Self::NoStore(err.to_string()),
            keyring::Error::NoEntry => Self::NotFound,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Fetched at connect time, never cached. Implementations must not add a
/// method that returns anything shaped like "the last password we
/// fetched" — that would reintroduce the exact plaintext-persistence risk
/// `ConnectionMetadata`'s type shape exists to rule out.
pub trait CredentialStore: Send + Sync {
    fn store(&self, password_ref: &PasswordRef, password: &str)
        -> Result<(), CredentialStoreError>;
    fn fetch(&self, password_ref: &PasswordRef) -> Result<String, CredentialStoreError>;
    fn delete(&self, password_ref: &PasswordRef) -> Result<(), CredentialStoreError>;
}

pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, password_ref: &PasswordRef) -> Result<keyring::Entry, CredentialStoreError> {
        Ok(keyring::Entry::new(SERVICE, &password_ref.0)?)
    }
}

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn store(
        &self,
        password_ref: &PasswordRef,
        password: &str,
    ) -> Result<(), CredentialStoreError> {
        self.entry(password_ref)?.set_password(password)?;
        Ok(())
    }

    fn fetch(&self, password_ref: &PasswordRef) -> Result<String, CredentialStoreError> {
        Ok(self.entry(password_ref)?.get_password()?)
    }

    fn delete(&self, password_ref: &PasswordRef) -> Result<(), CredentialStoreError> {
        self.entry(password_ref)?.delete_credential()?;
        Ok(())
    }
}
