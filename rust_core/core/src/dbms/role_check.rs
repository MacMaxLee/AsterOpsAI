//! Least-privilege validation (requirement 4): refuses a superuser
//! connection when `environment == Production` unless the connection's own
//! `allow_superuser_override` is explicitly set — and even then, the
//! override is meant to be audited (`audit_event_for_override`, wired to
//! `core::repository::record_audit_event` by whoever holds a
//! `RepositoryHandle`; this module stays decoupled from the repository
//! layer's specifics, matching its own narrow job).
//!
//! Documents the expected non-override role grant here rather than only in
//! a comment nobody reads at connect time:
//!
//! ```sql
//! GRANT pg_monitor TO ai_ops_readonly;
//! GRANT pg_read_all_stats TO ai_ops_readonly;
//! ```

use chrono::{DateTime, Utc};

use super::adapters::postgresql::queries;
use super::connection_metadata::Environment;
use super::DbmsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationOutcome {
    pub rolsuper: bool,
    /// True only when the connection *was* a superuser, environment was
    /// `Production`, and the connection's explicit override let it through
    /// anyway — the one case that needs an audit row.
    pub override_used: bool,
}

pub async fn validate(
    client: &tokio_postgres::Client,
    environment: Environment,
    allow_superuser_override: bool,
) -> Result<ValidationOutcome, DbmsError> {
    let rolsuper = queries::current_user_is_superuser(client).await?;

    if rolsuper && environment == Environment::Production {
        if !allow_superuser_override {
            return Err(DbmsError::PermissionDenied(
                "connection uses a superuser role against a PRODUCTION-classified \
                 instance; refusing without an explicit override (grant pg_monitor + \
                 pg_read_all_stats to a dedicated role instead)"
                    .to_string(),
            ));
        }
        return Ok(ValidationOutcome {
            rolsuper: true,
            override_used: true,
        });
    }

    Ok(ValidationOutcome {
        rolsuper,
        override_used: false,
    })
}

/// Builds the audit-log entry for a used superuser override; the caller
/// sends it through `core::repository::record_audit_event` (this module
/// has no `RepositoryHandle` of its own — see the module doc comment).
pub fn audit_event_for_override(
    connection_name: &str,
    now: DateTime<Utc>,
) -> crate::repository::NewAuditEvent {
    crate::repository::NewAuditEvent {
        ts: now,
        event_type: "DBMS_SUPERUSER_OVERRIDE".to_string(),
        actor: "system:dbms-connect".to_string(),
        summary: format!(
            "connection '{connection_name}' used a superuser role against a \
             PRODUCTION-classified instance via an explicit override"
        ),
        detail_json: serde_json::json!({ "connection_name": connection_name }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_event_names_the_connection() {
        let event = audit_event_for_override("prod-primary", Utc::now());
        assert_eq!(event.event_type, "DBMS_SUPERUSER_OVERRIDE");
        assert!(event.summary.contains("prod-primary"));
        assert!(event.detail_json.contains("prod-primary"));
    }
}
