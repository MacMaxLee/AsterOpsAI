//! `known_guc_values` (migrations/V14, unit U55): durable "what was
//! this GUC's setting the last time we polled it" tracking — the
//! diff-against-a-prior-snapshot half of SRS FR-DBSEC-001(e)'s
//! configuration-change detector. Mirrors `device_trust.rs`'s exact
//! shape.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::time::format_ts;

/// Atomically (single writer thread, ADR 0007) reads the last-known
/// setting for `name` (if any), then upserts `setting` as the new
/// last-known value. Returns the *previous* value: `None` means this
/// is the first time `name` has ever been observed — a real baseline
/// seed, not a change — the same "empty history isn't a real event"
/// precedent `device_trust::record_seen` already established.
pub fn record_guc_value(
    conn: &Connection,
    name: &str,
    setting: &str,
    now: DateTime<Utc>,
) -> Result<Option<String>, RepositoryError> {
    let previous: Option<String> = conn
        .query_row(
            "SELECT setting FROM known_guc_values WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )
        .optional()?;

    conn.execute(
        "INSERT INTO known_guc_values (name, setting, updated_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT(name) DO UPDATE SET setting = excluded.setting, updated_at = excluded.updated_at",
        params![name, setting, format_ts(now)],
    )?;

    Ok(previous)
}
