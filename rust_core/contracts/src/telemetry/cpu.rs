use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::metric_value::MetricValue;
use super::pressure::CpuPressure;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CpuSnapshot {
    pub timestamp: DateTime<Utc>,
    pub logical_core_count: u32,
    pub aggregate_utilization_percent: MetricValue<f64>,
    /// Index `i` is core `i`.
    pub per_core_utilization_percent: Vec<MetricValue<f64>>,
    /// Index `i` is core `i`. `Unavailable` on hosts without cpufreq
    /// exposure (e.g. many VMs) — never a fabricated value.
    pub frequency_mhz: Vec<MetricValue<f64>>,
    pub load_average_1m: MetricValue<f64>,
    pub load_average_5m: MetricValue<f64>,
    pub load_average_15m: MetricValue<f64>,
    pub context_switches_per_sec: MetricValue<f64>,
    pub interrupts_per_sec: MetricValue<f64>,
    pub pressure: CpuPressure,
    pub containerized: bool,
}
