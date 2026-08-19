use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single telemetry number, tagged with why it might not be a plain value.
/// Used for both genuine rates (CPU%, network throughput) and point-in-time
/// gauges (memory used, disk free) — gauges simply never produce the
/// `SampleGap`/`CounterReset` variants. Deliberately separate from
/// `SelfMetricValue<T>` (U0's service self-monitoring type); unifying the two
/// is a candidate for a later cleanup unit, not this one.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetricValue<T> {
    Supported {
        value: T,
    },
    /// Elapsed time between samples exceeded 2x the configured sample
    /// interval — a rate computed across that gap would be misleading.
    SampleGap {
        reason: String,
    },
    /// The underlying counter decreased between samples (NIC reinit,
    /// process restart, etc.) — never reported as a negative rate.
    CounterReset {
        reason: String,
    },
    Unavailable {
        reason: String,
    },
}
