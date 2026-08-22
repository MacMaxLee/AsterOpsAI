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
