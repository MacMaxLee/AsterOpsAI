//! `tuning_plans` (migrations/V11, unit U10). `insert_tuning_plan` maps a
//! violation of the partial `UNIQUE` index
//! (`idx_tuning_plans_one_in_flight_per_target`) to a dedicated
//! `RepositoryError` variant rather than the generic `Query` catch-all —
//! same precedent as `repository::policy::transition`'s own precise error
//! discrimination — since `core::tuning` needs to distinguish "a plan is
//! already in flight" (FR-TUNE-002, expected and handled) from any other
//! insert failure.

use rusqlite::{params, Connection, ErrorCode, OptionalExtension};

use super::error::RepositoryError;
use super::models::{NewTuningPlan, TuningPlanRow};
use super::time::{format_ts, parse_ts};

const SELECT_COLUMNS: &str = "id, created_at, target_identity_json, target_start_time, \
     profile, mode, status, completed_at, candidates_json";

fn row_from_sqlite(row: &rusqlite::Row) -> rusqlite::Result<TuningPlanRow> {
    Ok(TuningPlanRow {
        id: row.get(0)?,
        created_at: parse_ts(&row.get::<_, String>(1)?),
        target_identity_json: row.get(2)?,
        target_start_time: row.get(3)?,
        profile: row.get(4)?,
        mode: row.get(5)?,
        status: row.get(6)?,
        completed_at: row.get::<_, Option<String>>(7)?.map(|s| parse_ts(&s)),
        candidates_json: row.get(8)?,
    })
}

pub fn get_by_id(conn: &Connection, id: i64) -> Result<Option<TuningPlanRow>, RepositoryError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM tuning_plans WHERE id = ?1");
    conn.query_row(&sql, params![id], row_from_sqlite)
        .optional()
        .map_err(Into::into)
}

fn must_get_by_id(conn: &Connection, id: i64) -> Result<TuningPlanRow, RepositoryError> {
    get_by_id(conn, id)?.ok_or(RepositoryError::TuningPlanNotFound(id))
}

/// Unit U14's tuning plan history view: the most recent plans, newest
/// first — a history view reads from the most recent backward, the
/// opposite ordering from `policy::list_pending_approval`'s own
/// oldest-first inbox. `tuning_plans` has no retention policy (unlike
/// telemetry rollups), so this is capped at a real, explicit limit
/// rather than returning an unbounded, ever-growing result set.
const RECENT_PLANS_LIMIT: i64 = 100;

pub fn list_recent(conn: &Connection) -> Result<Vec<TuningPlanRow>, RepositoryError> {
    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM tuning_plans ORDER BY created_at DESC LIMIT ?1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![RECENT_PLANS_LIMIT], row_from_sqlite)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// A constraint-violation `rusqlite::Error` is `SqliteFailure` with a
/// primary `ErrorCode::ConstraintViolation` — the only `UNIQUE`/`CHECK`
/// index this insert can hit is `idx_tuning_plans_one_in_flight_per_target`
/// (migrations/V11), so the primary code alone is precise enough here.
fn is_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(inner, _) if inner.code == ErrorCode::ConstraintViolation
    )
}

pub fn insert_tuning_plan(
    conn: &Connection,
    new: &NewTuningPlan,
) -> Result<TuningPlanRow, RepositoryError> {
    let result = conn.execute(
        "INSERT INTO tuning_plans (
            created_at, target_identity_json, target_start_time,
            profile, mode, status, candidates_json
        ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            format_ts(new.created_at),
            new.target_identity_json,
            new.target_start_time,
            new.profile,
            new.mode,
            new.status,
            new.candidates_json,
        ],
    );
    match result {
        Ok(_) => must_get_by_id(conn, conn.last_insert_rowid()),
        Err(err) if is_constraint_violation(&err) => {
            Err(RepositoryError::TuningPlanAlreadyInFlight)
        }
        Err(err) => Err(err.into()),
    }
}

pub fn mark_plan_completed(
    conn: &Connection,
    id: i64,
    status: &str,
    completed_at: chrono::DateTime<chrono::Utc>,
    candidates_json: &str,
) -> Result<TuningPlanRow, RepositoryError> {
    conn.execute(
        "UPDATE tuning_plans SET status = ?1, completed_at = ?2, candidates_json = ?3 \
         WHERE id = ?4",
        params![status, format_ts(completed_at), candidates_json, id],
    )?;
    must_get_by_id(conn, id)
}
