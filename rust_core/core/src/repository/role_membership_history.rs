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

/// Unit U74 (ADR 0063's own named gap): forgets any `(member,
/// granted_role)` pair this sweep no longer observed — i.e. a real
/// revocation. Without this, a tuple once known stays known forever,
/// so a revoke followed by a genuine re-grant would never fire again.
/// `current` is the complete, freshly-polled membership set for this
/// sweep tick, not an incremental delta. Returns the number of stale
/// pairs forgotten (test/observability convenience, not load-bearing).
pub fn reconcile(
    conn: &Connection,
    current: &[(String, String)],
) -> Result<usize, RepositoryError> {
    let mut stmt = conn.prepare("SELECT member_role, granted_role FROM known_role_memberships")?;
    let known: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let mut forgotten = 0usize;
    for (member, granted_role) in known {
        if !current
            .iter()
            .any(|(m, g)| *m == member && *g == granted_role)
        {
            conn.execute(
                "DELETE FROM known_role_memberships \
                 WHERE member_role = ?1 AND granted_role = ?2",
                params![member, granted_role],
            )?;
            forgotten += 1;
        }
    }
    Ok(forgotten)
}
