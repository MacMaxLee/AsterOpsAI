use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::pressure::{CpuPressure, MemoryPressure};
use crate::capability::{Capability, CapabilityFamily};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SystemStatusResponse {
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub containerized: bool,
    pub cpu_pressure: CpuPressure,
    pub memory_pressure: MemoryPressure,
    /// The host telemetry sampler's current tick interval — 1000ms
    /// normally, backed off to 5000ms under HIGH/CRITICAL CPU pressure.
    pub sample_interval_ms: u64,
    pub capabilities: BTreeMap<CapabilityFamily, Capability>,
}
