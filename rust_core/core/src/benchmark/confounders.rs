//! TRS §34's five named confounders. Three have no real data source
//! anywhere in this codebase — same honestly-flagged gap `analysis`'s
//! ADR 0010 already documented for THERMAL/POWER host bottlenecks (no
//! sensor exposure on any platform) and for "other active tuning plans"
//! (TRS §36's tuning-plan concept doesn't exist yet, confirmed downstream
//! of this unit). They stay `None`, never fabricated.

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SampleGap {
    pub after: DateTime<Utc>,
    pub before: DateTime<Utc>,
    pub gap_seconds: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Confounders {
    /// Real, computed: `process_total_count` at the end of the window
    /// minus at the start — read from whatever `MetricSampler` context
    /// callers attach, or left `None` when a caller doesn't track it.
    pub process_count_delta: Option<i64>,
    /// No thermal sensor data source exists on any platform yet (see
    /// analysis's docs/adr/0010) — always `None`.
    pub thermal_state: Option<String>,
    /// No power/battery data source exists yet either — always `None`.
    pub ac_battery_transition: Option<bool>,
    /// No tuning-plan concept exists yet (TRS §36, downstream of this
    /// unit) — always `None`.
    pub active_tuning_plans: Option<u32>,
    /// Real, computed: gaps between consecutive poll timestamps
    /// exceeding `2x` the configured poll interval, in either window.
    pub sample_gaps: Vec<SampleGap>,
}
