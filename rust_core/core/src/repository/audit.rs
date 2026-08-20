//! Tamper-evident audit hash chain (requirement 6). `row_hash =
//! sha256(prev_hash || canonical_encoding(row))`, hand-rolled and
//! length-prefixed rather than relying on `serde_json`'s field-order
//! guarantee — that guarantee holds today for `#[derive(Serialize)]`
//! structs, but would silently break if a future edit routed `detail_json`
//! through a `HashMap`/`serde_json::Value` map, whose key order isn't
//! canonical. `id` is part of the hash, so rows can't be silently reordered
//! without invalidating the chain.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use super::error::RepositoryError;
use super::models::{AuditEventRecorded, CanonicalAuditRow, ChainVerification, NewAuditEvent};
use super::time::format_ts;

/// Named constant, never all-zero bytes, so an uninitialized value can never
/// be mistaken for a legitimate seed.
const GENESIS_SEED: &[u8] = b"AsterOpsAI-audit-chain-genesis-v1";

pub fn genesis_hash() -> String {
    hex_encode(&Sha256::digest(GENESIS_SEED))
}

/// Unit U13: the most-recently-inserted audit event's `event_type`, if
/// any — lets a caller confirm which specific event a just-completed
/// action wrote (e.g. distinguishing `"policy.rejected"` from
/// `"policy.denied"`) without needing its own SQL.
pub fn latest_event_type(conn: &Connection) -> Result<Option<String>, RepositoryError> {
    conn.query_row(
        "SELECT event_type FROM audit_events ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn write_len_prefixed(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    buf.extend_from_slice(bytes);
}

fn canonical_bytes(prev_hash: &str, row: &CanonicalAuditRow<'_>) -> Vec<u8> {
    let mut buf = Vec::new();
    write_len_prefixed(&mut buf, prev_hash.as_bytes());
    buf.extend_from_slice(&row.id.to_be_bytes());
    // Hash the same millisecond-precision *string* that's actually
    // persisted (via `format_ts`) — not the full nanosecond-precision
    // in-memory `DateTime`. Hashing the unrounded value here would make
    // every row's hash unreproducible at verify time, since reading the
    // row back necessarily loses the sub-millisecond digits that were
    // never stored in the first place.
    write_len_prefixed(&mut buf, format_ts(row.ts).as_bytes());
    write_len_prefixed(&mut buf, row.event_type.as_bytes());
    write_len_prefixed(&mut buf, row.actor.as_bytes());
    write_len_prefixed(&mut buf, row.summary.as_bytes());
    write_len_prefixed(&mut buf, row.detail_json.as_bytes());
    buf
}

pub fn compute_row_hash(prev_hash: &str, row: &CanonicalAuditRow<'_>) -> String {
    hex_encode(&Sha256::digest(canonical_bytes(prev_hash, row)))
}

/// Seeds the writer thread's in-memory chain state at startup: the next id
/// to assign and the hash to chain from. Single writer thread means this is
/// the only place these ever need to be read from the database.
pub fn seed_chain_state(conn: &Connection) -> Result<(i64, String), RepositoryError> {
    let last: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, row_hash FROM audit_events ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(match last {
        Some((last_id, last_hash)) => (last_id + 1, last_hash),
        None => (1, genesis_hash()),
    })
}

/// Inserts one audit event with an explicit, caller-supplied `id` and
/// `prev_hash` — both known before this call because there is exactly one
/// writer thread tracking them locally (see `writer.rs`), so no query is
/// needed and no race is possible.
pub fn insert_audit_event(
    conn: &Connection,
    id: i64,
    prev_hash: &str,
    new: &NewAuditEvent,
) -> Result<AuditEventRecorded, RepositoryError> {
    let canonical = CanonicalAuditRow {
        id,
        ts: new.ts,
        event_type: &new.event_type,
        actor: &new.actor,
        summary: &new.summary,
        detail_json: &new.detail_json,
    };
    let row_hash = compute_row_hash(prev_hash, &canonical);

    conn.execute(
        "INSERT INTO audit_events (id, ts, event_type, actor, summary, detail_json, prev_hash, row_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            format_ts(new.ts),
            new.event_type,
            new.actor,
            new.summary,
            new.detail_json,
            prev_hash,
            row_hash,
        ],
    )?;

    Ok(AuditEventRecorded { id, row_hash })
}

/// Walks the chain in insertion order and reports the `id` of the first row
/// whose `row_hash` doesn't match its recomputed hash, if any. Callable from
/// a read-only connection even while the service is running — WAL's whole
/// point is concurrent readers alongside the one writer.
pub fn verify_chain(conn: &Connection) -> Result<ChainVerification, RepositoryError> {
    let mut stmt = conn.prepare(
        "SELECT id, ts, event_type, actor, summary, detail_json, prev_hash, row_hash \
         FROM audit_events ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;

    let mut expected_prev = genesis_hash();
    let mut total_rows = 0u64;
    let mut first_broken_id = None;

    for row in rows {
        let (id, ts_text, event_type, actor, summary, detail_json, prev_hash, row_hash) = row?;
        total_rows += 1;
        if first_broken_id.is_some() {
            continue;
        }

        if prev_hash != expected_prev {
            first_broken_id = Some(id);
            continue;
        }

        let ts: DateTime<Utc> = match DateTime::parse_from_rfc3339(&ts_text) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => {
                first_broken_id = Some(id);
                continue;
            }
        };

        let canonical = CanonicalAuditRow {
            id,
            ts,
            event_type: &event_type,
            actor: &actor,
            summary: &summary,
            detail_json: &detail_json,
        };
        let recomputed = compute_row_hash(&prev_hash, &canonical);
        if recomputed != row_hash {
            first_broken_id = Some(id);
            continue;
        }

        expected_prev = row_hash;
    }

    Ok(ChainVerification {
        total_rows,
        first_broken_id,
    })
}
