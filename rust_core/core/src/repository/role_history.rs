//! `known_role_superuser_flags` (migrations/V15, unit U56): durable
//! "what was this role's `rolsuper` flag the last time we polled it"
//! tracking — the diff-against-a-prior-snapshot half of SRS
//! FR-DBSEC-001(b)'s superuser-grant detector, narrowed to the
//! `rolsuper` flag specifically. Mirrors `guc_history.rs`'s exact
//! shape.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::time::format_ts;

/// Atomically (single writer thread, ADR 0007) reads the last-known
/// `rolsuper` flag for `rolname` (if any), then upserts the current
/// flag as the new last-known value. Returns the *previous* flag:
/// `None` means this is the first time `rolname` has ever been
/// observed — a real baseline seed, not a change — the same
/// "empty history isn't a real event" precedent `record_guc_value`
/// already established.
pub fn record_role_superuser_flag(
    conn: &Connection,
    rolname: &str,
    rolsuper: bool,
    now: DateTime<Utc>,
) -> Result<Option<bool>, RepositoryError> {
    let previous: Option<i64> = conn
        .query_row(
            "SELECT rolsuper FROM known_role_superuser_flags WHERE rolname = ?1",
            params![rolname],
            |row| row.get(0),
        )
        .optional()?;

    conn.execute(
        "INSERT INTO known_role_superuser_flags (rolname, rolsuper, updated_at) \
         VALUES (?1, ?2, ?3) \
         ON CONFLICT(rolname) DO UPDATE SET rolsuper = excluded.rolsuper, \
         updated_at = excluded.updated_at",
        params![rolname, rolsuper, format_ts(now)],
    )?;

    Ok(previous.map(|v| v != 0))
}
