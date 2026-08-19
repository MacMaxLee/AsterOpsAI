use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Deterministic memory pressure tier. Thresholds are documented in
/// docs/adr/0006-pressure-classification-thresholds.md — SRS FR-MEM-001
/// requires this classification but pins no exact numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryPressure {
    Normal,
    Elevated,
    High,
    Critical,
}

/// Deterministic CPU pressure tier, parallel to `MemoryPressure`. Drives the
/// host telemetry sampler's back-off (unit U1 requirement 8). Thresholds
/// documented alongside memory's in ADR 0006.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CpuPressure {
    Normal,
    Elevated,
    High,
    Critical,
}
