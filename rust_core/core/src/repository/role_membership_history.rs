//! `known_role_memberships` (migrations/V17, unit U58): durable "have
//! we ever seen this exact `(member, granted_role)` pair before"
//! tracking — the remainder of SRS FR-DBSEC-001(b) unit U56 (ADR
//! 0061) explicitly deferred (general role-membership grants via
//! `pg_auth_members`, not just the `rolsuper` flag). Mirrors
//! `device_trust.rs`'s exact shape, just with a composite key.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::error::RepositoryError;
use super::time::format_ts;

/// Atomically (single writer thread, ADR 0007) checks whether the
/// exact `(member, granted_role)` pair was already known, and records
/// it as known if not. Returns whether it was *already* known before
/// this call.
pub fn record_seen(
    conn: &Connection,
    member: &str,
    granted_role: &str,
    now: DateTime<Utc>,
) -> Result<bool, RepositoryError> {
    let already_known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM known_role_memberships \
         WHERE member_role = ?1 AND granted_role = ?2)",
        params![member, granted_role],
        |row| row.get::<_, i64>(0),
    )? != 0;

    if !already_known {
        conn.execute(
            "INSERT INTO known_role_memberships (member_role, granted_role, first_seen_at) \
             VALUES (?1, ?2, ?3)",
            params![member, granted_role, format_ts(now)],
        )?;
    }

    Ok(already_known)
}
