use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::models::{NewProposedAction, PolicyActionRow, TransitionPatch};
use super::time::{format_ts, parse_ts};

const SELECT_COLUMNS: &str = "id, created_at, action_type, target_identity_json, target_start_time, \
     risk_classification, status, approved_by, executed_at, rollback_of, evidence_json, result_json, \
     requested_by, parameters_json, parameters_hash, resource_descriptor_json, approval_expires_at";

fn row_from_sqlite(row: &rusqlite::Row) -> rusqlite::Result<PolicyActionRow> {
    Ok(PolicyActionRow {
        id: row.get(0)?,
        created_at: parse_ts(&row.get::<_, String>(1)?),
        action_type: row.get(2)?,
        target_identity_json: row.get(3)?,
        target_start_time: row.get(4)?,
        risk_classification: row.get(5)?,
        status: row.get(6)?,
        approved_by: row.get(7)?,
        executed_at: row.get::<_, Option<String>>(8)?.map(|s| parse_ts(&s)),
        rollback_of: row.get(9)?,
        evidence_json: row.get(10)?,
        result_json: row.get(11)?,
        requested_by: row.get(12)?,
        parameters_json: row.get(13)?,
        parameters_hash: row.get(14)?,
        resource_descriptor_json: row.get(15)?,
        approval_expires_at: row.get::<_, Option<String>>(16)?.map(|s| parse_ts(&s)),
    })
}

pub fn get_by_id(conn: &Connection, id: i64) -> Result<Option<PolicyActionRow>, RepositoryError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM actions WHERE id = ?1");
    conn.query_row(&sql, params![id], row_from_sqlite)
        .optional()
        .map_err(Into::into)
}

fn must_get_by_id(conn: &Connection, id: i64) -> Result<PolicyActionRow, RepositoryError> {
    get_by_id(conn, id)?.ok_or(RepositoryError::PolicyActionNotFound(id))
}

/// Inserts a new action-lifecycle row — either a fresh proposal
/// (`new.rollback_of` is `None`) or a rollback attempt of a previously
/// executed action (`Some(original_id)`), both going through the same row
/// shape per this unit's "one row is one lifecycle instance" design.
pub fn insert_proposed_action(
    conn: &Connection,
    new: &NewProposedAction,
) -> Result<PolicyActionRow, RepositoryError> {
    conn.execute(
        "INSERT INTO actions (
            created_at, action_type, target_identity_json, target_start_time,
            risk_classification, status, evidence_json, requested_by,
            parameters_json, parameters_hash, resource_descriptor_json,
            approval_expires_at, rollback_of
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            format_ts(new.created_at),
            new.action_type,
            new.target_identity_json,
            new.target_start_time,
            new.risk_classification,
            new.status,
            new.evidence_json,
            new.requested_by,
            new.parameters_json,
            new.parameters_hash,
            new.resource_descriptor_json,
            new.approval_expires_at.map(format_ts),
            new.rollback_of,
        ],
    )?;
    must_get_by_id(conn, conn.last_insert_rowid())
}

/// Atomic compare-and-swap lifecycle transition: `UPDATE ... WHERE id = ?
/// AND status = ?expected`. The single writer thread (ADR 0007) means this
/// is race-free without any extra locking — no two callers can ever both
/// see `affected == 1` for the same `(id, expected_status)` pair, which is
/// exactly what makes an approval single-use. `check_not_expired`
/// additionally requires `approval_expires_at IS NULL OR
/// approval_expires_at > now`. Zero rows affected is turned into a precise
/// reason (not found / wrong status / expired) by a follow-up read, not
/// returned as one generic failure.
#[allow(clippy::too_many_arguments)]
pub fn transition(
    conn: &Connection,
    id: i64,
    expected_status: &str,
    new_status: &str,
    now: DateTime<Utc>,
    check_not_expired: bool,
    patch: &TransitionPatch,
) -> Result<PolicyActionRow, RepositoryError> {
    // Two full, independent SQL + param sets (not one SQL string shared with
    // a variable-length params! call) — `?N` placeholder count and bound
    // value count must match exactly, and the two branches genuinely have a
    // different placeholder count (7 vs. 6).
    let affected = if check_not_expired {
        conn.execute(
            "UPDATE actions SET status = ?1, \
             approved_by = COALESCE(?2, approved_by), \
             executed_at = COALESCE(?3, executed_at), \
             result_json = COALESCE(?4, result_json) \
             WHERE id = ?5 AND status = ?6 \
             AND (approval_expires_at IS NULL OR approval_expires_at > ?7)",
            params![
                new_status,
                patch.approved_by,
                patch.executed_at.map(format_ts),
                patch.result_json,
                id,
                expected_status,
                format_ts(now),
            ],
        )?
    } else {
        conn.execute(
            "UPDATE actions SET status = ?1, \
             approved_by = COALESCE(?2, approved_by), \
             executed_at = COALESCE(?3, executed_at), \
             result_json = COALESCE(?4, result_json) \
             WHERE id = ?5 AND status = ?6",
            params![
                new_status,
                patch.approved_by,
                patch.executed_at.map(format_ts),
                patch.result_json,
                id,
                expected_status,
            ],
        )?
    };
    if affected == 0 {
        let row = must_get_by_id(conn, id)?;
        return Err(if row.status != expected_status {
            RepositoryError::PolicyActionWrongStatus {
                id,
                expected: expected_status.to_string(),
                actual: row.status,
            }
        } else {
            // Status matched but the row still didn't update — only
            // possible when `check_not_expired` rejected it on expiry.
            RepositoryError::PolicyActionExpired(id)
        });
    }
    must_get_by_id(conn, id)
}
