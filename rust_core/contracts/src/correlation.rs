//! Wire types for U5/U12's cross-layer correlation verdict (unit U20).
//! Mirror `core::correlation::{cause, hypothesis}` field-for-field — this
//! crate has no workspace-internal dependencies, so it can't reuse those
//! types directly. `Evidence` is not redefined here — it's the same type
//! `contracts::analysis` already promoted in U19.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::analysis::Evidence;

/// SRS FR-CORR-002's exact nine-value list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RootCause {
    DbLocks,
    DbConfiguration,
    ConnectionExhaustion,
    SlowSql,
    HostCpu,
    HostMemory,
    StorageLatency,
    Network,
    ClientSideApplication,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Hypothesis {
    pub cause: RootCause,
    pub confidence: f64,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuledOut {
    pub cause: RootCause,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CorrelationResult {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    /// Sorted descending by `confidence`.
    pub ranked: Vec<Hypothesis>,
    pub ruled_out: Vec<RuledOut>,
}
