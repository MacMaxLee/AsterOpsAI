//! AI provider config resolution (unit U44), mirroring `dbms_config.rs`'s
//! own plain-env-var, all-or-nothing precedent exactly: `$ASTEROPS_AI_MODEL`
//! is required (its presence is what "AI is configured" means — a wrong
//! guessed model name would just fail confusingly at the provider, so this
//! never invents a default the way `AiProviderConfig::new`'s own caller is
//! expected to supply one); `$ASTEROPS_AI_BASE_URL`/`$ASTEROPS_AI_TIMEOUT_SECS`
//! are optional and fall back to `AiProviderConfig::new`'s own defaults.

use std::time::Duration;

use ai_ops_core::ai::AiProviderConfig;

fn non_empty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.is_empty())
}

/// `None` means "no AI provider configured" — a real, honest degrade the
/// `/analysis/host/explain` endpoint treats as `503 Unavailable`, not a
/// half-built config with a guessed model name.
pub fn resolve_ai_config_from(
    model: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<String>,
) -> Option<AiProviderConfig> {
    let model = non_empty(model)?;
    let mut config = AiProviderConfig::new(model);
    if let Some(base_url) = non_empty(base_url) {
        config.base_url = base_url;
    }
    if let Some(secs) = non_empty(timeout_secs).and_then(|s| s.parse::<u64>().ok()) {
        config.timeout = Duration::from_secs(secs);
    }
    Some(config)
}

pub fn resolve_ai_config() -> Option<AiProviderConfig> {
    resolve_ai_config_from(
        std::env::var("ASTEROPS_AI_MODEL").ok(),
        std::env::var("ASTEROPS_AI_BASE_URL").ok(),
        std::env::var("ASTEROPS_AI_TIMEOUT_SECS").ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_model_means_no_ai_provider_configured() {
        assert!(resolve_ai_config_from(None, None, None).is_none());
    }

    #[test]
    fn an_empty_model_is_treated_as_unset() {
        assert!(resolve_ai_config_from(Some(String::new()), None, None).is_none());
    }

    #[test]
    fn a_model_alone_fills_conventional_defaults() {
        let cfg = resolve_ai_config_from(Some("llama3".into()), None, None).unwrap();
        assert_eq!(cfg.model, "llama3");
        assert_eq!(cfg.base_url, "http://127.0.0.1:11434");
        assert_eq!(cfg.timeout, Duration::from_secs(10));
    }

    #[test]
    fn explicit_fields_override_the_defaults() {
        let cfg = resolve_ai_config_from(
            Some("llama3".into()),
            Some("http://10.0.0.5:11434".into()),
            Some("30".into()),
        )
        .unwrap();
        assert_eq!(cfg.base_url, "http://10.0.0.5:11434");
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }

    #[test]
    fn an_unparsable_timeout_falls_back_to_the_default_rather_than_erroring() {
        let cfg = resolve_ai_config_from(Some("llama3".into()), None, Some("not-a-number".into()))
            .unwrap();
        assert_eq!(cfg.timeout, Duration::from_secs(10));
    }
}
