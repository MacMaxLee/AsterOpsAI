//! Target identity (TRS §27). Stored as `target_identity_json` +
//! `target_start_time` on the `actions` row (migrations/V4); the JSON form
//! is what equality/deviation checks compare, `target_start_time` is a
//! denormalized copy of whichever start-time field the variant carries,
//! kept as its own indexed column per V4's own `idx_actions_target_start_time`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// TRS §27's exact two kinds. Process targets by `(pid, start_time_ticks)`
/// (matching `ProcessInfo.start_time_ticks`'s `/proc/[pid]/stat` field-22
/// semantics, unit U1); DB session targets by `(backend_pid,
/// backend_start)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TargetIdentity {
    Process {
        pid: u32,
        start_time_ticks: u64,
    },
    DbSession {
        backend_pid: i32,
        backend_start: DateTime<Utc>,
    },
}

impl TargetIdentity {
    /// The value stored in `actions.target_start_time` (an `INTEGER`
    /// column) — a process's raw tick count, or a DB session's start time
    /// as a Unix timestamp, so both variants have one comparable/indexable
    /// column regardless of kind.
    pub fn start_time_marker(&self) -> i64 {
        match self {
            Self::Process {
                start_time_ticks, ..
            } => *start_time_ticks as i64,
            Self::DbSession { backend_start, .. } => backend_start.timestamp(),
        }
    }
}
