//! Connection metadata (TRS §18, SRS NFR-PRIV-002): everything needed to
//! reach a PostgreSQL instance *except* the password, which this type is
//! structurally incapable of holding — there is no `password: String`
//! field to accidentally populate, serialize, or log. The password lives
//! only in the OS credential store (`credential_store.rs`) and is fetched
//! at connect time.

/// Mirrors `tokio_postgres::config::SslMode` exactly — that type has only
/// these three variants (`Disable`/`Prefer`/`Require`), not the full six-mode
/// `libpq` `sslmode` set. `verify-ca`/`verify-full` need a CA bundle path
/// plus a certificate-verifying TLS connector wired in on top of this
/// (`tokio-postgres` is connector-agnostic; cert verification is configured
/// on whatever connector you hand `connect()`, not on this enum) — real,
/// separate work with its own config surface (a CA path field
/// `ConnectionMetadata` doesn't have yet), not something to fake by adding
/// variants this code can't actually honor. Deliberately scoped down to what
/// `pool.rs` genuinely implements this unit; TRS §17's "an explicit sslmode"
/// requirement is about never silently defaulting/omitting one, which
/// `Disable`/`Require` already satisfy for real.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    Disable,
    Prefer,
    Require,
}

impl From<TlsMode> for tokio_postgres::config::SslMode {
    fn from(mode: TlsMode) -> Self {
        match mode {
            TlsMode::Disable => Self::Disable,
            TlsMode::Prefer => Self::Prefer,
            TlsMode::Require => Self::Require,
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
    /// Refuses a superuser connection in `Production` unless set —
    /// `role_check.rs` audits every connection made with this set to true.
    pub allow_superuser_override: bool,
    /// TRS §19: raw SQL is captured only on this explicit per-connection
    /// opt-in; every connection defaults to normalized-query-only capture.
    pub capture_raw_sql: bool,
}
