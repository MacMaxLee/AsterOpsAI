use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::models::{NewProposedAction, PolicyActionRow, TransitionPatch};
use super::time::{format_ts, parse_ts};

const SELECT_COLUMNS: &str = "id, created_at, action_type, target_identity_json, target_start_time, \
     risk_classification, status, approved_by, executed_at, rollback_of, evidence_json, result_json, \
     requested_by, parameters_json, parameters_hash, resource_descriptor_json, approval_expires_at, \
     previous_state_json";

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
        previous_state_json: row.get(17)?,
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

/// Unit U13's policy approval inbox: every row still awaiting a human
/// decision, oldest first — a real inbox drains from the front, not an
/// arbitrary default. Unit U50 (ADR 0028/0055): also includes
/// `AUTO_ALLOWED` rows — a real, audited row policy already authorized
/// but that (outside `core::tuning::plan.rs`'s own narrower
/// auto-execution gate) nothing ever executes on its own; a human still
/// needs a real trigger to run it, so it belongs in the same inbox, not
/// a separate surface.
pub fn list_pending_approval(conn: &Connection) -> Result<Vec<PolicyActionRow>, RepositoryError> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM actions \
         WHERE status IN ('PENDING_APPROVAL', 'AUTO_ALLOWED') \
         ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], row_from_sqlite)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Unit U29: the one `EXECUTED` action against this exact target that
/// hasn't already been successfully rolled back, if any — what a real
/// "Resume" affordance needs to find. `rollback()` never mutates the
/// original row (a successful rollback inserts a *new* row with
/// `rollback_of` set instead), so a plain `status = 'EXECUTED'` filter
/// alone would keep offering "Resume" forever even after a process was
/// already resumed — the `NOT IN` subquery excludes any row that
/// already has a `ROLLED_BACK` child. A `FAILED` rollback attempt does
/// *not* exclude the original: a failed attempt means the target is
/// presumably still in its executed (e.g. suspended) state, so it must
/// stay resumable. Most recent first, one row: as of unit U43,
/// `insert_proposed_action` below refuses a second fresh proposal of
/// the *same `action_type`* against the same target while an
/// unresolved one already exists, so double-suspending the same target
/// can no longer happen. This query still stays "most recent wins"
/// rather than "assert exactly one", though: it has no `action_type`
/// filter, so two different action types both `EXECUTED`-and-not-yet-
/// rolled-back against the same target (e.g. a suspend and a separate
/// priority change) can legitimately coexist — the guard only prevents
/// duplicates of the *same* action type, matching ADR 0034/0048's own
/// scoping.
pub fn find_resumable_action(
    conn: &Connection,
    target_identity_json: &str,
    target_start_time: i64,
) -> Result<Option<PolicyActionRow>, RepositoryError> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM actions \
         WHERE status = 'EXECUTED' AND target_identity_json = ?1 AND target_start_time = ?2 \
         AND id NOT IN (SELECT rollback_of FROM actions \
             WHERE rollback_of IS NOT NULL AND status = 'ROLLED_BACK') \
         ORDER BY created_at DESC LIMIT 1"
    );
    conn.query_row(
        &sql,
        params![target_identity_json, target_start_time],
        row_from_sqlite,
    )
    .optional()
    .map_err(Into::into)
}

/// Unit U43 (ADR 0034/0048): true if this exact `(action_type,
/// target_identity_json, target_start_time)` already has an unresolved
/// action — either a genuinely in-flight status, or an `EXECUTED` row
/// with no `ROLLED_BACK` child yet (`find_resumable_action`'s own
/// predicate, inlined rather than reused directly, since it doesn't
/// filter by `action_type`).
fn has_unresolved_action(
    conn: &Connection,
    action_type: &str,
    target_identity_json: &str,
    target_start_time: i64,
) -> Result<bool, RepositoryError> {
    let sql = "SELECT EXISTS (\
        SELECT 1 FROM actions \
        WHERE action_type = ?1 AND target_identity_json = ?2 AND target_start_time = ?3 \
        AND ( \
            status IN ('AUTO_ALLOWED', 'PENDING_APPROVAL', 'APPROVED', 'EXECUTING') \
            OR (status = 'EXECUTED' AND id NOT IN ( \
                SELECT rollback_of FROM actions \
                WHERE rollback_of IS NOT NULL AND status = 'ROLLED_BACK' \
            )) \
        ) \
    )";
    conn.query_row(
        sql,
        params![action_type, target_identity_json, target_start_time],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

/// Inserts a new action-lifecycle row — either a fresh proposal
/// (`new.rollback_of` is `None`) or a rollback attempt of a previously
/// executed action (`Some(original_id)`), both going through the same row
/// shape per this unit's "one row is one lifecycle instance" design.
///
/// A fresh proposal (never a rollback attempt — that row's whole point
/// is to resolve an existing unresolved one) is rejected outright if
/// this target already has an unresolved action of the same
/// `action_type` (unit U43, closing ADR 0034's own named gap). Checked
/// on this same connection immediately before the `INSERT`, with no
/// `.await` in between — race-free because both run synchronously
/// inside the single writer thread (ADR 0007), the same guarantee
/// `tuning_plans`' own partial `UNIQUE` index relies on, just enforced
/// in application code since this predicate (a correlated subquery)
/// can't be expressed as a SQLite partial-index predicate.
pub fn insert_proposed_action(
    conn: &Connection,
    new: &NewProposedAction,
) -> Result<PolicyActionRow, RepositoryError> {
    if new.rollback_of.is_none()
        && has_unresolved_action(
            conn,
            &new.action_type,
            &new.target_identity_json,
            new.target_start_time,
        )?
    {
        return Err(RepositoryError::ConflictingActionInFlight);
    }
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
             result_json = COALESCE(?4, result_json), \
             previous_state_json = COALESCE(?5, previous_state_json) \
             WHERE id = ?6 AND status = ?7 \
             AND (approval_expires_at IS NULL OR approval_expires_at > ?8)",
            params![
                new_status,
                patch.approved_by,
                patch.executed_at.map(format_ts),
                patch.result_json,
                patch.previous_state_json,
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
             result_json = COALESCE(?4, result_json), \
             previous_state_json = COALESCE(?5, previous_state_json) \
             WHERE id = ?6 AND status = ?7",
            params![
                new_status,
                patch.approved_by,
                patch.executed_at.map(format_ts),
                patch.result_json,
                patch.previous_state_json,
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
