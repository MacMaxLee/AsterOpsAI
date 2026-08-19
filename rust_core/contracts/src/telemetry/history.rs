use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which persisted rollup tier a history query actually resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoryTier {
    Raw,
    Hourly,
    Daily,
}

/// One metric's statistics over a bucket. Raw-tier points reuse this same
/// shape (`avg == min == max ==` the single value, `supported_sample_count
/// ∈ {0, 1}`) so the console never has to branch on tier shape (FR-CONSOLE-001).
/// `avg`/`min`/`max` are `None` when `supported_sample_count == 0` — never a
/// fabricated number for an all-gap/all-unavailable bucket.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistoryStat {
    pub avg: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub supported_sample_count: u32,
    pub total_sample_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CpuHistoryPoint {
    pub bucket_start: DateTime<Utc>,
    pub bucket_end: DateTime<Utc>,
    pub aggregate_utilization_percent: HistoryStat,
    pub load_average_1m: HistoryStat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryHistoryPoint {
    pub bucket_start: DateTime<Utc>,
    pub bucket_end: DateTime<Utc>,
    pub used_bytes: HistoryStat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StorageHistoryPoint {
    pub bucket_start: DateTime<Utc>,
    pub bucket_end: DateTime<Utc>,
    pub read_bytes_per_sec: HistoryStat,
    pub write_bytes_per_sec: HistoryStat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NetworkHistoryPoint {
    pub bucket_start: DateTime<Utc>,
    pub bucket_end: DateTime<Utc>,
    pub rx_bytes_per_sec: HistoryStat,
    pub tx_bytes_per_sec: HistoryStat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistoryResponse<T> {
    pub requested_from: DateTime<Utc>,
    pub requested_to: DateTime<Utc>,
    pub resolved_tier: HistoryTier,
    /// Set when the requested range was clamped (e.g. `from` older than the
    /// 2-year daily-rollup retention horizon).
    pub truncated_from: Option<DateTime<Utc>>,
    pub points: Vec<T>,
}
