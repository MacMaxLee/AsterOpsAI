//! `known_client_addresses` (migrations/V16, unit U57): durable "have
//! we ever seen a connection from this client address before"
//! tracking — SRS FR-DBSEC-001(d)'s unusual-client-address detector.
//! Mirrors `device_trust.rs`'s exact shape.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::error::RepositoryError;
use super::time::format_ts;

/// Atomically (single writer thread, ADR 0007 — no concurrent writer
/// can interleave between the `SELECT` and the `INSERT`) checks
/// whether `client_addr` was already known, and records it as known
/// if not. Returns whether it was *already* known before this call.
pub fn record_seen(
    conn: &Connection,
    client_addr: &str,
    now: DateTime<Utc>,
) -> Result<bool, RepositoryError> {
    let already_known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM known_client_addresses WHERE client_addr = ?1)",
        params![client_addr],
        |row| row.get::<_, i64>(0),
    )? != 0;

    if !already_known {
        conn.execute(
            "INSERT INTO known_client_addresses (client_addr, first_seen_at) VALUES (?1, ?2)",
            params![client_addr, format_ts(now)],
        )?;
    }

    Ok(already_known)
}
