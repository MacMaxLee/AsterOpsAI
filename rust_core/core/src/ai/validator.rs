//! The mechanical citation check TRS §23 names as the real security
//! boundary — not keyword-based prompt-injection filtering. Every numeric
//! claim must resolve into the `EvidenceBundle` that was actually sent
//! (SRS FR-AI-005); every candidate reference must resolve the same way
//! (FR-AI-004). One bad citation anywhere discards the *whole* response —
//! this function either returns a fully-trustworthy `AiExplanation` or an
//! error, never a partially-trusted value.

use super::bundle::EvidenceBundle;
use super::provider::AiError;
use super::schema::{MetricClaim, RawAiExplanation, RiskLevel};

const ALLOWED_RISK_LEVELS: [&str; 4] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

fn check_metrics(metrics: &[MetricClaim], bundle: &EvidenceBundle) -> Result<(), AiError> {
    for m in metrics {
        if !bundle.evidence.iter().any(|e| e.id == m.evidence_ref) {
            return Err(AiError::SchemaValidation(format!(
                "evidence_ref {} does not resolve into the evidence bundle",
                m.evidence_ref
            )));
        }
    }
    Ok(())
}

pub fn validate(
    raw: RawAiExplanation,
    bundle: &EvidenceBundle,
) -> Result<super::schema::AiExplanation, AiError> {
    if !(0.0..=1.0).contains(&raw.confidence) {
        return Err(AiError::SchemaValidation(format!(
            "confidence {} is outside [0.0, 1.0]",
            raw.confidence
        )));
    }
    if !ALLOWED_RISK_LEVELS.contains(&raw.risk.as_str()) {
        return Err(AiError::SchemaValidation(format!(
            "risk {:?} is not one of {ALLOWED_RISK_LEVELS:?}",
            raw.risk
        )));
    }
    let risk = RiskLevel::parse(&raw.risk).ok_or_else(|| {
        AiError::SchemaValidation(format!(
            "risk {:?} failed to parse after allow-list check",
            raw.risk
        ))
    })?;

    for observation in &raw.observations {
        check_metrics(&observation.metrics, bundle)?;
    }
    for recommendation in &raw.recommendations {
        check_metrics(&recommendation.metrics, bundle)?;
        if let Some(candidate_ref) = recommendation.candidate_ref {
            if !bundle.candidates.iter().any(|c| c.id == candidate_ref) {
                return Err(AiError::SchemaValidation(format!(
                    "candidate_ref {candidate_ref} does not resolve into the evidence bundle"
                )));
            }
        }
    }

    Ok(super::schema::AiExplanation {
        summary: raw.summary,
        observations: raw.observations,
        recommendations: raw.recommendations,
        risk,
        confidence: raw.confidence,
    })
}
