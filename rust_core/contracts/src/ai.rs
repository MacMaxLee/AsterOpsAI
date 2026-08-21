//! Unit U44's first direct wire surface for `core::ai` — fully built
//! since unit U6 (SRS FR-AI-001..005), but until now consumed by
//! nothing in `service` at all. Deliberately parallel to
//! `core::ai::schema`'s types, not re-exported (`contracts` has no
//! workspace-internal deps, CLAUDE.md), mirroring every other
//! `contracts` module's own convention.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MetricClaim {
    pub value: f64,
    pub evidence_ref: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    pub text: String,
    pub metrics: Vec<MetricClaim>,
}

/// The one response shape allowed to reference a `Candidate` id — never
/// a free-form identifier the model invented (SRS FR-AI-004).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Recommendation {
    pub text: String,
    pub metrics: Vec<MetricClaim>,
    pub candidate_ref: Option<u32>,
}

/// Only ever produced server-side by `core::ai::validator::validate` —
/// its existence on the wire is the proof a value already passed
/// citation checking (SRS FR-AI-005) before this endpoint ever saw it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiExplanation {
    pub summary: String,
    pub observations: Vec<Observation>,
    pub recommendations: Vec<Recommendation>,
    pub risk: RiskLevel,
    pub confidence: f64,
}
