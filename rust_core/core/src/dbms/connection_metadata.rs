//! Connection metadata (TRS §18, SRS NFR-PRIV-002): everything needed to
//! reach a PostgreSQL instance *except* the password, which this type is
//! structurally incapable of holding — there is no `password: String`
//! field to accidentally populate, serialize, or log. The password lives
//! only in the OS credential store (`credential_store.rs`) and is fetched
//! at connect time.

use std::path::PathBuf;

/// `tokio_postgres::config::SslMode` only has three variants (`Disable`/
/// `Prefer`/`Require`) — it governs the startup-handshake TLS *requirement*
/// only, not certificate verification (`tokio-postgres` is connector-
/// agnostic; verification is configured on whatever connector you hand
/// `connect()`). `VerifyCa`/`VerifyFull` (unit U77, closing ADR 0009's own
/// named gap) both map to `Require` there — real verification behavior
/// lives entirely in `pool::build_pool`'s own choice of TLS connector, not
/// in this handshake-mode flag. `VerifyCa` requires a real, current CA
/// chain trust check but deliberately tolerates a hostname mismatch;
/// `VerifyFull` requires both — matching `libpq`'s own documented
/// `sslmode` semantics for these two modes exactly. Both require
/// `ca_bundle_path` to be set (see `ConnectionMetadata`); `pool::
/// build_pool` returns a real, explicit error rather than silently
/// downgrading verification if it's missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl From<TlsMode> for tokio_postgres::config::SslMode {
    fn from(mode: TlsMode) -> Self {
        match mode {
            TlsMode::Disable => Self::Disable,
            TlsMode::Prefer => Self::Prefer,
            TlsMode::Require | TlsMode::VerifyCa | TlsMode::VerifyFull => Self::Require,
        }
    }
}

/// Matches the least-privilege check in `role_check.rs`: a superuser
/// connection is refused in `Production` unless explicitly overridden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

/// An opaque lookup key into the credential store — never the secret
/// itself. Deliberately has no `Display`/custom `Debug` beyond the
/// derived one (which prints the lookup key, a stable identifier, not a
/// password) so a stray `{:?}` in a log line can't leak anything.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PasswordRef(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionMetadata {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub tls_mode: TlsMode,
    pub environment: Environment,
    pub password_ref: PasswordRef,
    /// Required (and read) only for `TlsMode::VerifyCa`/`VerifyFull` — a
    /// PEM file of one or more CA certificates to trust, mirroring
    /// `libpq`'s own `sslrootcert`. Unused (may be `None`) for every other
    /// mode.
    pub ca_bundle_path: Option<PathBuf>,
    /// Refuses a superuser connection in `Production` unless set —
    /// `role_check.rs` audits every connection made with this set to true.
    pub allow_superuser_override: bool,
    /// TRS §19: raw SQL is captured only on this explicit per-connection
    /// opt-in; every connection defaults to normalized-query-only capture.
    pub capture_raw_sql: bool,
}
