//! `known_table_privilege_grants` (migrations/V18, unit U59): durable
//! "have we ever seen this exact `(grantee, schema, table,
//! privilege_type)` tuple before" tracking — SRS FR-DBSEC-001(c)'s
//! table permission-grant detector, deliberately narrowed (see
//! docs/adr/0064). Mirrors `role_membership_history.rs`'s exact
//! shape, just with a wider composite key.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::error::RepositoryError;
use super::time::format_ts;

/// Atomically (single writer thread, ADR 0007) checks whether the
/// exact `(grantee, schema, table, privilege_type)` tuple was already
/// known, and records it as known if not. Returns whether it was
/// *already* known before this call.
pub fn record_seen(
    conn: &Connection,
    grantee: &str,
    schema: &str,
    table: &str,
    privilege_type: &str,
    now: DateTime<Utc>,
) -> Result<bool, RepositoryError> {
    let already_known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM known_table_privilege_grants \
         WHERE grantee = ?1 AND table_schema = ?2 AND table_name = ?3 AND privilege_type = ?4)",
        params![grantee, schema, table, privilege_type],
        |row| row.get::<_, i64>(0),
    )? != 0;

    if !already_known {
        conn.execute(
            "INSERT INTO known_table_privilege_grants \
             (grantee, table_schema, table_name, privilege_type, first_seen_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![grantee, schema, table, privilege_type, format_ts(now)],
        )?;
    }

    Ok(already_known)
}

/// Unit U74 (ADR 0064's own named gap, mirrors `role_membership_
/// history::reconcile` exactly): forgets any `(grantee, schema, table,
/// privilege_type)` tuple this sweep no longer observed — i.e. a real
/// revocation — so a revoke followed by a genuine re-grant fires again
/// instead of staying silently suppressed forever. `current` is the
/// complete, freshly-polled grant set for this sweep tick. Returns the
/// number of stale tuples forgotten (test/observability convenience).
pub fn reconcile(
    conn: &Connection,
    current: &[(String, String, String, String)],
) -> Result<usize, RepositoryError> {
    let mut stmt = conn.prepare(
        "SELECT grantee, table_schema, table_name, privilege_type \
         FROM known_table_privilege_grants",
    )?;
    let known: Vec<(String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let mut forgotten = 0usize;
    for (grantee, schema, table, privilege_type) in known {
        let still_present = current.iter().any(|(g, s, t, p)| {
            *g == grantee && *s == schema && *t == table && *p == privilege_type
        });
        if !still_present {
            conn.execute(
                "DELETE FROM known_table_privilege_grants \
                 WHERE grantee = ?1 AND table_schema = ?2 AND table_name = ?3 \
                 AND privilege_type = ?4",
                params![grantee, schema, table, privilege_type],
            )?;
            forgotten += 1;
        }
    }
    Ok(forgotten)
}
