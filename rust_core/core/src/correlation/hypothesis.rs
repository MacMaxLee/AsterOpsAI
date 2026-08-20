use chrono::{DateTime, Utc};

use crate::analysis::Evidence;

use super::cause::RootCause;

/// SRS FR-CORR-001: a ranked root-cause hypothesis, with its own
/// supporting evidence and a confidence value.
#[derive(Debug, Clone)]
pub struct Hypothesis {
    pub cause: RootCause,
    pub confidence: f64,
    pub evidence: Vec<Evidence>,
}

/// A cause considered but rejected, with the reason why — FR-CORR-001's
/// explicit requirement, not left implicit in an empty `ranked` slot.
#[derive(Debug, Clone)]
pub struct RuledOut {
    pub cause: RootCause,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CorrelationResult {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    /// Sorted descending by `confidence`.
    pub ranked: Vec<Hypothesis>,
    pub ruled_out: Vec<RuledOut>,
}
