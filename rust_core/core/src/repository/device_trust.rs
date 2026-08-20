//! `known_devices` (migrations/V12, unit U11): durable "have we ever seen
//! this device identifier before" tracking, replacing the service
//! sampler's own ephemeral, in-process `HashSet` (which reset on every
//! restart — see `contracts::telemetry::device::DeviceInfo.trusted`'s own
//! doc comment).

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::error::RepositoryError;
use super::time::format_ts;

/// Atomically (single writer thread, ADR 0007 — no concurrent writer can
/// interleave between the `SELECT` and the `INSERT`) checks whether
/// `identifier` was already known, and records it as known if not.
/// Returns whether it was *already* known before this call.
pub fn record_seen(
    conn: &Connection,
    identifier: &str,
    now: DateTime<Utc>,
) -> Result<bool, RepositoryError> {
    let already_known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM known_devices WHERE identifier = ?1)",
        params![identifier],
        |row| row.get::<_, i64>(0),
    )? != 0;

    if !already_known {
        conn.execute(
            "INSERT INTO known_devices (identifier, first_seen_at) VALUES (?1, ?2)",
            params![identifier, format_ts(now)],
        )?;
    }

    Ok(already_known)
}
