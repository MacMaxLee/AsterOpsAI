//! The `AiProvider` trait (TRS §22) and the degrade-gracefully entry point
//! every caller outside this module should actually use. SRS FR-AI-001:
//! every other subsystem must function with the provider absent,
//! unreachable, slow, or returning garbage — `try_explain` is where that
//! guarantee lives, mirroring `repository::try_persist_telemetry_snapshot`'s
//! own "never propagate, log and degrade" shape.

use std::time::Duration;

use async_trait::async_trait;

use super::bundle::EvidenceBundle;
use super::schema::AiExplanation;

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("failed to connect to the AI provider: {0}")]
    Connect(String),

    #[error("AI provider request timed out")]
    Timeout,

    #[error("AI provider returned a non-success HTTP status: {0}")]
    HttpStatus(u16),

    #[error("failed to parse the AI provider's response as JSON: {0}")]
    InvalidJson(String),

    #[error("AI response failed schema/citation validation: {0}")]
    SchemaValidation(String),
}

#[derive(Debug, Clone)]
pub struct AiProviderConfig {
    pub base_url: String,
    /// No hardcoded default — a specific model name pretending to be
    /// installed would just fail with a confusing provider-side error;
    /// the caller must say which model it expects to be pulled.
    pub model: String,
    pub timeout: Duration,
}

impl AiProviderConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".to_string(),
            model: model.into(),
            timeout: Duration::from_secs(10),
        }
    }
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn explain(&self, bundle: &EvidenceBundle) -> Result<AiExplanation, AiError>;
}

/// The entry point every caller outside `core::ai` should use instead of
/// `AiProvider::explain` directly — any failure (connect, timeout, garbage
/// output, a citation that doesn't resolve) is logged and degrades to
/// `None`, never propagated or panicking.
pub async fn try_explain(
    provider: &dyn AiProvider,
    bundle: &EvidenceBundle,
) -> Option<AiExplanation> {
    match provider.explain(bundle).await {
        Ok(explanation) => Some(explanation),
        Err(err) => {
            tracing::warn!(error = %err, subject = %bundle.subject, "AI explanation unavailable");
            None
        }
    }
}
