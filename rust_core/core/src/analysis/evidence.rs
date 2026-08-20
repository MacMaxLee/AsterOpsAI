//! Shared evidence type (SRS FR-PERF-002): every classification — host
//! bottleneck or DB health check — carries a list of these. A verdict with
//! an empty evidence list is a bug in the classifier that produced it, not
//! a valid "trust me" result.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub metric: String,
    pub observed: f64,
    pub threshold: f64,
    pub unit: Option<String>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

impl Evidence {
    pub fn new(
        metric: impl Into<String>,
        observed: f64,
        threshold: f64,
        unit: Option<&str>,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> Self {
        Self {
            metric: metric.into(),
            observed,
            threshold,
            unit: unit.map(str::to_string),
            window_start,
            window_end,
        }
    }
}
