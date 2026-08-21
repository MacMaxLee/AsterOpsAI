//! Unit U25: which real `core::policy::risk::Environment` this deployment
//! runs as — the input to `core::policy::risk::decide`'s risk decision
//! (SRS FR-POL-002), consumed by `api/v1/tuning.rs`'s `start` handler.
//! Structurally unrelated to `core::dbms::connection_metadata::Environment`
//! (`dbms_config.rs`'s own, separate concern — a DB connection label, not
//! a risk-decision input); this module has nothing to do with that one.

use ai_ops_core::policy::Environment;

/// Unset, empty, or unrecognized all fall back to `Development` — the
/// same "unparsable falls back to the default, never errors" judgment
/// call `dbms_config::parse_tls_mode` already makes, and the same safe
/// default (most permissive) this exact call site was hardcoded to
/// before this unit, so an unconfigured deployment behaves identically
/// to before.
fn resolve_policy_environment_from(raw: Option<String>) -> Environment {
    match raw.as_deref() {
        Some("staging") => Environment::Staging,
        Some("production") => Environment::Production,
        _ => Environment::Development,
    }
}

pub fn resolve_policy_environment() -> Environment {
    resolve_policy_environment_from(std::env::var("ASTEROPS_POLICY_ENVIRONMENT").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_falls_back_to_development() {
        assert_eq!(
            resolve_policy_environment_from(None),
            Environment::Development
        );
    }

    #[test]
    fn empty_falls_back_to_development() {
        assert_eq!(
            resolve_policy_environment_from(Some(String::new())),
            Environment::Development
        );
    }

    #[test]
    fn an_unrecognized_value_falls_back_to_development_rather_than_erroring() {
        assert_eq!(
            resolve_policy_environment_from(Some("nonsense".into())),
            Environment::Development
        );
    }

    #[test]
    fn staging_is_recognized() {
        assert_eq!(
            resolve_policy_environment_from(Some("staging".into())),
            Environment::Staging
        );
    }

    #[test]
    fn production_is_recognized() {
        assert_eq!(
            resolve_policy_environment_from(Some("production".into())),
            Environment::Production
        );
    }

    #[test]
    fn development_is_recognized_explicitly_too() {
        assert_eq!(
            resolve_policy_environment_from(Some("development".into())),
            Environment::Development
        );
    }
}
