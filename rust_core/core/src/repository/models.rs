use chrono::{DateTime, Utc};
use serde::Serialize;

/// One persisted sampler tick. Field names mirror `telemetry_snapshots`'
/// columns directly. Built from `HostTelemetrySnapshot` by
/// `telemetry_store::snapshot_to_row` (in the `service` crate, since that's
/// where `HostTelemetrySnapshot` lives) — this crate only knows about the
/// row shape, never the live telemetry contract types, keeping
/// `core::repository` decoupled from `core::telemetry`.
#[derive(Debug, Clone)]
pub struct TelemetrySnapshotRow {
    pub ts: DateTime<Utc>,

    pub cpu_aggregate_util_pct: Option<f64>,
    pub cpu_aggregate_util_state: String,
    pub cpu_load_avg_1m: Option<f64>,
    pub cpu_pressure: String,
    pub cpu_per_core_json: Option<String>,

    pub mem_total_bytes: Option<i64>,
    pub mem_used_bytes: Option<i64>,
    pub mem_used_bytes_state: String,
    pub mem_available_bytes: Option<i64>,
    pub mem_swap_used_bytes: Option<i64>,
    pub mem_pressure: String,

    pub storage_read_bytes_ps: Option<f64>,
    pub storage_write_bytes_ps: Option<f64>,
    pub storage_volumes_json: Option<String>,

    pub net_rx_bytes_ps: Option<f64>,
    pub net_tx_bytes_ps: Option<f64>,
    pub net_interfaces_json: Option<String>,

    pub process_total_count: Option<i64>,
    pub device_count: Option<i64>,

    pub containerized: bool,
}

/// What a caller supplies to record an audit event; `id`, `prev_hash`, and
/// `row_hash` are computed by the writer thread, never by the caller.
#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub ts: DateTime<Utc>,
    pub event_type: String,
    pub actor: String,
    pub summary: String,
    pub detail_json: String,
}

#[derive(Debug, Clone)]
pub struct AuditEventRecorded {
    pub id: i64,
    pub row_hash: String,
}

/// The exact fields hashed into `row_hash`, in a fixed, documented order.
/// `id` is included so rows can't be silently reordered without invalidating
/// the chain.
pub struct CanonicalAuditRow<'a> {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub event_type: &'a str,
    pub actor: &'a str,
    pub summary: &'a str,
    pub detail_json: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ChainVerification {
    pub total_rows: u64,
    /// The `id` of the first row whose `row_hash` doesn't match its
    /// recomputed hash, if any.
    pub first_broken_id: Option<i64>,
}

/// One row for the `performance_analysis` table (migrations/V5, unit U5).
/// `subject` distinguishes what was classified — `"HOST"` for a host
/// bottleneck verdict, or a DB connection's name for a DB health verdict —
/// and `score_version` is one of `analysis::HOST_SCORE_VERSION` /
/// `analysis::DB_SCORE_VERSION`, never mutated in place (SRS FR-PERF-004).
#[derive(Debug, Clone)]
pub struct PerformanceAnalysisRow {
    pub ts: DateTime<Utc>,
    pub subject: String,
    pub score: f64,
    pub score_version: String,
    pub verdict: String,
    pub evidence_json: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RetentionAuditDetail {
    pub rows_deleted_raw: u64,
    pub rows_deleted_hourly: u64,
    pub rows_deleted_daily: u64,
    pub hourly_rollups_computed: u64,
    pub daily_rollups_computed: u64,
    pub db_size_before_bytes: u64,
    pub db_size_after_bytes: u64,
    pub ceiling_bytes: u64,
    pub ceiling_breached: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RetentionReport {
    pub detail: RetentionAuditDetail,
    pub audit_row: Option<AuditEventRecorded>,
}
