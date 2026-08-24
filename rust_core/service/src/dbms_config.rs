//! DB connection resolution for the correlation endpoint (unit U20).
//! Deliberately not the OS-keyring `CredentialStore` path
//! (`ai_ops_core::dbms::credential_store`) — that targets a desktop
//! session (Secret Service/Keychain), which a headless `service` process
//! may not have at all. `service` reads a plain env var instead
//! (`ASTEROPS_DB_PASSWORD`) and builds its own pool directly via
//! `dbms::pool::build_pool` — never touching `CredentialStore`. Same
//! category of deliberate, documented simplification as the "no auth/
//! session system exists yet" gap ADR 0021 already flagged for console
//! mutations, not a new one. See docs/adr/0025 for the full tradeoff.
//!
//! Unit U48 (ADR 0053) closes ADR 0025's own named gap: `environment`/
//! `allow_superuser_override` are now real, configurable fields
//! (`$ASTEROPS_DB_ENVIRONMENT`/`$ASTEROPS_DB_ALLOW_SUPERUSER_OVERRIDE`,
//! both previously hardcoded to `Development`/`false`), and
//! `dbms_connect::connect_and_validate` actually calls `role_check::
//! validate` against the pool this module builds — `PostgresAdapter::
//! from_pool` is still used (not `::connect`, which would require the
//! `CredentialStore` this module deliberately avoids), but the same
//! least-privilege check now runs regardless.

use ai_ops_core::dbms::{ConnectionMetadata, Environment, PasswordRef, TlsMode};

#[derive(Debug, Clone, PartialEq)]
pub struct DbConnectionConfig {
    pub metadata: ConnectionMetadata,
    pub password: String,
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.is_empty())
}

fn parse_tls_mode(v: Option<String>) -> TlsMode {
    match v.as_deref() {
        Some("require") => TlsMode::Require,
        Some("disable") => TlsMode::Disable,
        Some("verify-ca") => TlsMode::VerifyCa,
        Some("verify-full") => TlsMode::VerifyFull,
        _ => TlsMode::Prefer,
    }
}

/// Unset, empty, or unrecognized all fall back to `Development` — the
/// same judgment call `policy_config::resolve_policy_environment_from`
/// already makes for the structurally-unrelated `policy::risk::
/// Environment` (confirmed by direct read: that module's own doc
/// comment explicitly flags the two `Environment` types as unrelated,
/// so this is a genuinely separate config surface, not a reuse).
fn resolve_db_environment_from(v: Option<String>) -> Environment {
    match v.as_deref() {
        Some("staging") => Environment::Staging,
        Some("production") => Environment::Production,
        _ => Environment::Development,
    }
}

/// Unset, empty, or anything other than `"true"`/`"1"` means `false` —
/// the safe default (refuse a superuser role in Production rather than
/// silently permit one).
fn resolve_allow_superuser_override_from(v: Option<String>) -> bool {
    matches!(v.as_deref(), Some("true") | Some("1"))
}

/// All-or-nothing: an unset (or empty) `host` or `password` means no DB
/// is configured at all — never a half-built config with some fields
/// silently defaulted. A host with no password is not a usable
/// connection, so it's treated identically to being fully unset, not as
/// a distinct error state; `database`/`user`/`port`/`sslmode` fall back
/// to PostgreSQL's own conventional defaults when present-but-unparsable
/// or absent, since those alone don't make a connection unusable.
#[allow(clippy::too_many_arguments)]
pub fn resolve_db_connection_from(
    host: Option<String>,
    port: Option<String>,
    database: Option<String>,
    user: Option<String>,
    password: Option<String>,
    sslmode: Option<String>,
    environment: Option<String>,
    allow_superuser_override: Option<String>,
    ca_bundle_path: Option<String>,
) -> Option<DbConnectionConfig> {
    let host = non_empty(host)?;
    let password = non_empty(password)?;
    let database = non_empty(database).unwrap_or_else(|| "postgres".to_string());
    let username = non_empty(user).unwrap_or_else(|| "postgres".to_string());
    let port = non_empty(port)
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(5432);
    let tls_mode = parse_tls_mode(sslmode);
    let environment = resolve_db_environment_from(environment);
    let allow_superuser_override = resolve_allow_superuser_override_from(allow_superuser_override);
    let ca_bundle_path = non_empty(ca_bundle_path).map(std::path::PathBuf::from);

    Some(DbConnectionConfig {
        metadata: ConnectionMetadata {
            name: "asterops-correlation".to_string(),
            host,
            port,
            database,
            username,
            tls_mode,
            environment,
            password_ref: PasswordRef("service-env-var-unused".to_string()),
            ca_bundle_path,
            allow_superuser_override,
            capture_raw_sql: false,
        },
        password,
    })
}

pub fn resolve_db_connection() -> Option<DbConnectionConfig> {
    resolve_db_connection_from(
        std::env::var("ASTEROPS_DB_HOST").ok(),
        std::env::var("ASTEROPS_DB_PORT").ok(),
        std::env::var("ASTEROPS_DB_DATABASE").ok(),
        std::env::var("ASTEROPS_DB_USER").ok(),
        std::env::var("ASTEROPS_DB_PASSWORD").ok(),
        std::env::var("ASTEROPS_DB_SSLMODE").ok(),
        std::env::var("ASTEROPS_DB_ENVIRONMENT").ok(),
        std::env::var("ASTEROPS_DB_ALLOW_SUPERUSER_OVERRIDE").ok(),
        std::env::var("ASTEROPS_DB_CA_BUNDLE_PATH").ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_host_means_no_db_configured() {
        assert!(resolve_db_connection_from(
            None,
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            None,
            None,
        )
        .is_none());
    }

    #[test]
    fn a_host_with_no_password_means_no_db_configured() {
        assert!(resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .is_none());
    }

    #[test]
    fn an_empty_password_is_treated_as_unset() {
        assert!(resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some(String::new()),
            None,
            None,
            None,
            None,
        )
        .is_none());
    }

    #[test]
    fn host_and_password_alone_fill_conventional_defaults() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.host, "db.local");
        assert_eq!(cfg.metadata.port, 5432);
        assert_eq!(cfg.metadata.database, "postgres");
        assert_eq!(cfg.metadata.username, "postgres");
        assert_eq!(cfg.metadata.tls_mode, TlsMode::Prefer);
        assert_eq!(cfg.password, "secret");
        assert_eq!(cfg.metadata.environment, Environment::Development);
        assert!(!cfg.metadata.allow_superuser_override);
        assert_eq!(cfg.metadata.ca_bundle_path, None);
    }

    #[test]
    fn explicit_fields_override_the_defaults() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            Some("5433".into()),
            Some("appdb".into()),
            Some("dba".into()),
            Some("secret".into()),
            Some("disable".into()),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.port, 5433);
        assert_eq!(cfg.metadata.database, "appdb");
        assert_eq!(cfg.metadata.username, "dba");
        assert_eq!(cfg.metadata.tls_mode, TlsMode::Disable);
    }

    #[test]
    fn an_unparsable_port_falls_back_to_the_default_rather_than_erroring() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            Some("not-a-port".into()),
            None,
            None,
            Some("secret".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.port, 5432);
    }

    #[test]
    fn require_sslmode_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            Some("require".into()),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.tls_mode, TlsMode::Require);
    }

    /// Unit U77 (ADR 0009's own named gap, closed by docs/adr/0082).
    #[test]
    fn verify_ca_sslmode_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            Some("verify-ca".into()),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.tls_mode, TlsMode::VerifyCa);
    }

    #[test]
    fn verify_full_sslmode_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            Some("verify-full".into()),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.tls_mode, TlsMode::VerifyFull);
    }

    #[test]
    fn a_ca_bundle_path_is_resolved_when_given() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            Some("verify-full".into()),
            None,
            None,
            Some("/etc/asterops/ca.pem".into()),
        )
        .unwrap();
        assert_eq!(
            cfg.metadata.ca_bundle_path,
            Some(std::path::PathBuf::from("/etc/asterops/ca.pem"))
        );
    }

    #[test]
    fn an_empty_ca_bundle_path_is_treated_as_unset() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            None,
            Some(String::new()),
        )
        .unwrap();
        assert_eq!(cfg.metadata.ca_bundle_path, None);
    }

    #[test]
    fn unset_environment_falls_back_to_development() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.environment, Environment::Development);
    }

    #[test]
    fn an_unrecognized_environment_falls_back_to_development_rather_than_erroring() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            Some("not-a-real-environment".into()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.environment, Environment::Development);
    }

    #[test]
    fn production_environment_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            Some("production".into()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.environment, Environment::Production);
    }

    #[test]
    fn staging_environment_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            Some("staging".into()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.metadata.environment, Environment::Staging);
    }

    #[test]
    fn unset_allow_superuser_override_falls_back_to_false() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(!cfg.metadata.allow_superuser_override);
    }

    #[test]
    fn allow_superuser_override_true_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            Some("true".into()),
            None,
        )
        .unwrap();
        assert!(cfg.metadata.allow_superuser_override);
    }

    #[test]
    fn allow_superuser_override_1_is_recognized() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            Some("1".into()),
            None,
        )
        .unwrap();
        assert!(cfg.metadata.allow_superuser_override);
    }

    #[test]
    fn an_unrecognized_allow_superuser_override_falls_back_to_false() {
        let cfg = resolve_db_connection_from(
            Some("db.local".into()),
            None,
            None,
            None,
            Some("secret".into()),
            None,
            None,
            Some("yes-please".into()),
            None,
        )
        .unwrap();
        assert!(!cfg.metadata.allow_superuser_override);
    }
}
