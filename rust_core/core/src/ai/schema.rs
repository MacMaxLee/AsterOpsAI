//! The closed AI output schema (SRS FR-AI-003). `RawAiExplanation` is what
//! gets deserialized straight from the provider's JSON — plain `Deserialize`
//! derives, deliberately without `#[serde(deny_unknown_fields)]`: FR-AI-003's
//! "any field outside the schema is discarded" is exactly serde's default
//! behavior for an unrecognized field, not something this code needs to
//! special-case. `AiExplanation` is the same shape but only ever constructed
//! by `validator::validate` — its existence is the proof a value passed
//! citation checking.

use serde::Deserialize;

/// A numeric claim, always paired with the evidence entry it's citing
/// (SRS FR-AI-005). Numerics are confined to this dedicated field rather
/// than embedded in `text` prose specifically so a validator can check them
/// mechanically — see docs/adr/0011.
#[derive(Debug, Clone, Deserialize)]
pub struct MetricClaim {
    pub value: f64,
    pub evidence_ref: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Observation {
    pub text: String,
    #[serde(default)]
    pub metrics: Vec<MetricClaim>,
}

/// The one AI-response shape allowed to reference a `Candidate` — never a
/// free-form identifier the model invented (SRS FR-AI-004).
#[derive(Debug, Clone, Deserialize)]
pub struct Recommendation {
    pub text: String,
    #[serde(default)]
    pub metrics: Vec<MetricClaim>,
    #[serde(default)]
    pub candidate_ref: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawAiExplanation {
    pub summary: String,
    #[serde(default)]
    pub observations: Vec<Observation>,
    #[serde(default)]
    pub recommendations: Vec<Recommendation>,
    pub risk: String,
    pub confidence: f64,
}

/// Only ever produced by `validator::validate` — every `MetricClaim`/
/// `candidate_ref` in here has already been checked to resolve into the
/// `EvidenceBundle` that was sent to the provider.
#[derive(Debug, Clone)]
pub struct AiExplanation {
    pub summary: String,
    pub observations: Vec<Observation>,
    pub recommendations: Vec<Recommendation>,
    pub risk: RiskLevel,
    pub confidence: f64,
}

/// Advisory only — the AI layer has no execution path (FR-AI-002), so this
/// is never consulted by the policy engine's own risk classification
/// (SRS §17, a distinct, authoritative enum for a later unit). This is the
/// model's self-reported severity assessment of what it's describing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "LOW" => Some(Self::Low),
            "MEDIUM" => Some(Self::Medium),
            "HIGH" => Some(Self::High),
            "CRITICAL" => Some(Self::Critical),
            _ => None,
        }
    }
}
