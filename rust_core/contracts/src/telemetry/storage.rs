use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::metric_value::MetricValue;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StorageSnapshot {
    pub timestamp: DateTime<Utc>,
    pub volumes: Vec<VolumeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VolumeInfo {
    pub device: String,
    pub mount_point: String,
    pub filesystem: String,
    pub capacity_bytes: MetricValue<u64>,
    pub free_bytes: MetricValue<u64>,
    pub available_bytes: MetricValue<u64>,
    pub read_bytes_per_sec: MetricValue<f64>,
    pub write_bytes_per_sec: MetricValue<f64>,
    pub read_ops_per_sec: MetricValue<f64>,
    pub write_ops_per_sec: MetricValue<f64>,
    pub io_latency_ms: MetricValue<f64>,
}
