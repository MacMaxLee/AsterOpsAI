//! `log_tail_offsets` (migrations/V19, unit U60): durable "how far
//! into this log file have we already tailed" state for the
//! authentication-failure detector (`core::dbms::log_tail`). Unlike
//! every other `known_*`/history module in this directory, this is a
//! plain get/set pair rather than one atomic read-then-write function
//! — the real file I/O happens *between* the two calls, at the
//! `service` layer (read the log file starting at the offset `get_
//! offset` returns, then persist the new end-of-file position via
//! `set_offset`), so there's no single SQL statement to combine them
//! into.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::time::format_ts;

/// `None` means this log file has never been tailed before — the
/// caller should start reading from byte 0.
pub fn get_offset(conn: &Connection, log_file_path: &str) -> Result<Option<i64>, RepositoryError> {
    conn.query_row(
        "SELECT byte_offset FROM log_tail_offsets WHERE log_file_path = ?1",
        params![log_file_path],
        |row| row.get(0),
    )
    .optional()
    .map_err(RepositoryError::from)
}

pub fn set_offset(
    conn: &Connection,
    log_file_path: &str,
    byte_offset: i64,
    now: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    conn.execute(
        "INSERT INTO log_tail_offsets (log_file_path, byte_offset, updated_at) \
         VALUES (?1, ?2, ?3) \
         ON CONFLICT(log_file_path) DO UPDATE SET \
         byte_offset = excluded.byte_offset, updated_at = excluded.updated_at",
        params![log_file_path, byte_offset, format_ts(now)],
    )?;
    Ok(())
}
