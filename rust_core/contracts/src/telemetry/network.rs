use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::metric_value::MetricValue;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NetworkSnapshot {
    pub timestamp: DateTime<Utc>,
    pub interfaces: Vec<NetworkInterfaceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub rx_bytes_per_sec: MetricValue<f64>,
    pub tx_bytes_per_sec: MetricValue<f64>,
    pub rx_packets_per_sec: MetricValue<f64>,
    pub tx_packets_per_sec: MetricValue<f64>,
    pub rx_errors_per_sec: MetricValue<f64>,
    pub tx_errors_per_sec: MetricValue<f64>,
    pub rx_drops_per_sec: MetricValue<f64>,
    pub tx_drops_per_sec: MetricValue<f64>,
}
