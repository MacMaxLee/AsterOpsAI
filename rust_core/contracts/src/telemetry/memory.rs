use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::metric_value::MetricValue;
use super::pressure::MemoryPressure;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemorySnapshot {
    pub timestamp: DateTime<Utc>,
    /// Host total, or the cgroup v2 memory ceiling when `containerized`.
    pub total_bytes: MetricValue<u64>,
    /// `total_bytes - available_bytes`.
    pub used_bytes: MetricValue<u64>,
    pub available_bytes: MetricValue<u64>,
    pub cached_bytes: MetricValue<u64>,
    pub buffers_bytes: MetricValue<u64>,
    pub swap_total_bytes: MetricValue<u64>,
    pub swap_used_bytes: MetricValue<u64>,
    pub swap_free_bytes: MetricValue<u64>,
    pub pressure: MemoryPressure,
    pub containerized: bool,
    pub numa_nodes: MetricValue<Vec<NumaNodeMemory>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NumaNodeMemory {
    pub node_id: u32,
    pub total_bytes: u64,
    pub free_bytes: u64,
}
