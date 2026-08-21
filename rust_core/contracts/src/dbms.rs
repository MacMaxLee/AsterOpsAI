//! Unit U31's first direct wire surface for `core::dbms::DbmsAdapter`
//! (fully built, but until now consumed only internally by the
//! correlation endpoint's own ranked verdict — see docs/adr/0036).
//! `contracts` has no workspace-internal deps (CLAUDE.md), so these are
//! deliberately parallel types mirroring `core::dbms::{SessionState,
//! SessionInfo, LockEdge}` field-for-field, not re-exports.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionState {
    Active,
    Idle,
    IdleInTransaction,
    Waiting,
}

/// `query` is already sanitized (or capability-gated) by `core::dbms::
/// privacy::sanitize_query` before it ever reaches `core::dbms::
/// SessionInfo` — nothing about that policy is re-decided here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    pub pid: i32,
    pub username: Option<String>,
    pub database: Option<String>,
    pub state: SessionState,
    pub client_addr: Option<String>,
    pub xact_start: Option<DateTime<Utc>>,
    pub query_start: Option<DateTime<Utc>>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LockEdge {
    pub blocked_pid: i32,
    pub blocked_query: Option<String>,
    pub blocking_pid: i32,
    pub blocking_query: Option<String>,
    pub lock_type: String,
}

/// Unit U33: mirrors `core::dbms::QueryStat` field-for-field. Always
/// carried inside `GatedValue<Vec<QueryStat>>` (`contracts::
/// capability`), never returned bare — `pg_stat_statements` may
/// genuinely not be installed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryStat {
    pub query_fingerprint: String,
    pub normalized_query: String,
    pub calls: i64,
    pub total_exec_time_ms: f64,
    pub mean_exec_time_ms: f64,
    pub rows: i64,
}

/// Unit U35: mirrors `core::dbms::TableStat` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableStat {
    pub schema: String,
    pub table: String,
    pub seq_scan: i64,
    pub idx_scan: i64,
    pub n_live_tup: i64,
    pub n_dead_tup: i64,
    pub last_vacuum: Option<DateTime<Utc>>,
    pub last_autovacuum: Option<DateTime<Utc>>,
    pub total_size_bytes: i64,
}

/// Unit U35: mirrors `core::dbms::IndexStat` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexStat {
    pub schema: String,
    pub table: String,
    pub index: String,
    pub idx_scan: i64,
    pub size_bytes: i64,
}

/// Unit U37: mirrors `core::dbms::StandbyInfo` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StandbyInfo {
    pub client_addr: Option<String>,
    pub state: String,
    pub sent_lsn: Option<String>,
    pub write_lsn: Option<String>,
    pub flush_lsn: Option<String>,
    pub replay_lsn: Option<String>,
    pub replay_lag_seconds: Option<f64>,
}

/// Unit U37: mirrors `core::dbms::ReplicationStatus` field-for-field.
/// The first `/dbms/*` endpoint whose success payload is a single
/// object, not a `Vec<T>`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplicationStatus {
    pub is_primary: bool,
    pub in_recovery: bool,
    pub standbys: Vec<StandbyInfo>,
}

/// Unit U37: mirrors `core::dbms::GucValue` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GucValue {
    pub name: String,
    pub setting: String,
    pub unit: Option<String>,
    pub source: String,
}

/// Unit U39: mirrors `core::dbms::TempFileActivity` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TempFileActivity {
    pub temp_files: i64,
    pub temp_bytes: i64,
    pub stats_reset: Option<DateTime<Utc>>,
}

/// Unit U39: mirrors `core::dbms::DeadlockInfo` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeadlockInfo {
    pub deadlocks: i64,
    pub stats_reset: Option<DateTime<Utc>>,
}

/// Unit U41: mirrors `core::dbms::LongTransaction` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LongTransaction {
    pub pid: i32,
    pub username: Option<String>,
    pub duration_seconds: f64,
    pub state: SessionState,
    pub query: Option<String>,
}

/// Unit U41: mirrors `core::dbms::IdleInTransactionSession` field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IdleInTransactionSession {
    pub pid: i32,
    pub username: Option<String>,
    pub idle_duration_seconds: f64,
}
