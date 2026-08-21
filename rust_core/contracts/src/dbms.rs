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
