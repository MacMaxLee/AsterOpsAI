//! Wire types for U5's host performance analysis (unit U19). Mirror
//! `core::analysis::{host, thresholds, evidence}` field-for-field — this
//! crate has no workspace-internal dependencies, so it can't reuse those
//! types directly.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// SRS FR-PERF-001's exact ten values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HostBottleneck {
    None,
    Cpu,
    Memory,
    StorageIo,
    Network,
    Thermal,
    Power,
    Background,
    Multiple,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HostDomain {
    Cpu,
    Memory,
    StorageIo,
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Tier {
    Normal,
    Elevated,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub metric: String,
    pub observed: f64,
    pub threshold: f64,
    pub unit: Option<String>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DomainSignal {
    pub domain: HostDomain,
    pub tier: Tier,
    pub sample_count: u64,
    pub crossed_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HostVerdict {
    pub bottleneck: HostBottleneck,
    pub domain_signals: Vec<DomainSignal>,
    pub evidence: Vec<Evidence>,
    pub score: u8,
    pub score_version: String,
}
