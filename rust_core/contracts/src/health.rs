use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::capability::{Capability, CapabilityFamily};

/// Response body for `GET /api/v1/health`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HealthResponse {
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub uptime_seconds: u64,
    pub platform: String,
    pub arch: String,
    pub capabilities: BTreeMap<CapabilityFamily, Capability>,
    pub self_cpu_percent: SelfMetricValue<f64>,
    pub self_rss_bytes: SelfMetricValue<u64>,
}

/// A self-monitoring value that is capability-tagged rather than a bare
/// number, so `/health` stays a valid response on a platform where the
/// underlying metric isn't implemented yet — never a fabricated `0`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SelfMetricValue<T> {
    Supported { value: T },
    Unavailable { reason: String },
}
