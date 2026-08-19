use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::metric_value::MetricValue;
use crate::capability::Capability;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessSnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_count: u32,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessInfo {
    pub pid: u32,
    /// `/proc/[pid]/stat` field 22 — reserved for the target-identity
    /// re-verification scheme actions will need starting in unit U8; unused
    /// this unit.
    pub start_time_ticks: u64,
    pub comm: String,
    pub cmdline: MetricValue<String>,
    pub owner_uid: u32,
    pub cpu_percent: MetricValue<f64>,
    pub rss_bytes: MetricValue<u64>,
    pub category: ProcessCategory,
    pub disk_io_capability: Capability,
    pub disk_read_bytes_per_sec: MetricValue<f64>,
    pub disk_write_bytes_per_sec: MetricValue<f64>,
    pub network_io_capability: Capability,
    pub network_rx_bytes_per_sec: MetricValue<f64>,
    pub network_tx_bytes_per_sec: MetricValue<f64>,
}

/// SRS FR-PRO-003: deterministic process classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessCategory {
    System,
    UserApplication,
    BackgroundService,
    DbmsEngine,
    Unknown,
}
