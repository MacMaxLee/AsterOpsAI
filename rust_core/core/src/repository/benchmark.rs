use rusqlite::{params, Connection, OptionalExtension};

use super::error::RepositoryError;
use super::models::{BenchmarkRunRow, NewBenchmarkRun};
use super::time::{format_ts, parse_ts};

const SELECT_COLUMNS: &str = "id, created_at, action_id, metric_name, direction, \
     baseline_window_start, baseline_window_end, baseline_cv, \
     post_change_window_start, post_change_window_end, post_change_cv, \
     p_value, verdict, confounders_json, rolled_back, rollback_action_id";

fn row_from_sqlite(row: &rusqlite::Row) -> rusqlite::Result<BenchmarkRunRow> {
    Ok(BenchmarkRunRow {
        id: row.get(0)?,
        created_at: parse_ts(&row.get::<_, String>(1)?),
        action_id: row.get(2)?,
        metric_name: row.get(3)?,
        direction: row.get(4)?,
        baseline_window_start: parse_ts(&row.get::<_, String>(5)?),
        baseline_window_end: parse_ts(&row.get::<_, String>(6)?),
        baseline_cv: row.get(7)?,
        post_change_window_start: row.get::<_, Option<String>>(8)?.map(|s| parse_ts(&s)),
        post_change_window_end: row.get::<_, Option<String>>(9)?.map(|s| parse_ts(&s)),
        post_change_cv: row.get(10)?,
        p_value: row.get(11)?,
        verdict: row.get(12)?,
        confounders_json: row.get(13)?,
        rolled_back: row.get::<_, i64>(14)? != 0,
        rollback_action_id: row.get(15)?,
    })
}

pub fn get_by_id(conn: &Connection, id: i64) -> Result<Option<BenchmarkRunRow>, RepositoryError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM benchmark_runs WHERE id = ?1");
    conn.query_row(&sql, params![id], row_from_sqlite)
        .optional()
        .map_err(Into::into)
}

fn must_get_by_id(conn: &Connection, id: i64) -> Result<BenchmarkRunRow, RepositoryError> {
    get_by_id(conn, id)?.ok_or(RepositoryError::PolicyActionNotFound(id))
}

pub fn insert_benchmark_run(
    conn: &Connection,
    new: &NewBenchmarkRun,
) -> Result<BenchmarkRunRow, RepositoryError> {
    conn.execute(
        "INSERT INTO benchmark_runs (
            created_at, action_id, metric_name, direction,
            baseline_window_start, baseline_window_end, baseline_cv,
            post_change_window_start, post_change_window_end, post_change_cv,
            p_value, verdict, confounders_json
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            format_ts(new.created_at),
            new.action_id,
            new.metric_name,
            new.direction,
            format_ts(new.baseline_window_start),
            format_ts(new.baseline_window_end),
            new.baseline_cv,
            new.post_change_window_start.map(format_ts),
            new.post_change_window_end.map(format_ts),
            new.post_change_cv,
            new.p_value,
            new.verdict,
            new.confounders_json,
        ],
    )?;
    must_get_by_id(conn, conn.last_insert_rowid())
}

/// Unit U62 (SRS FR-HIST-002, narrowed — see docs/adr/0067): a real
/// compare-and-swap guard, the same shape `policy::transition` already
/// uses for `actions` rows — `rolled_back = 0` in the `WHERE` clause
/// means a second call against an already-rolled-back run affects zero
/// rows rather than silently overwriting `rollback_action_id`.
/// `must_get_by_id` itself reports a genuinely missing id (via `?`); a
/// row that's found here despite `affected == 0` can only have failed
/// the `rolled_back = 0` guard, i.e. it was already rolled back once.
pub fn mark_rolled_back(
    conn: &Connection,
    id: i64,
    rollback_action_id: i64,
) -> Result<BenchmarkRunRow, RepositoryError> {
    let affected = conn.execute(
        "UPDATE benchmark_runs SET rolled_back = 1, rollback_action_id = ?1 \
         WHERE id = ?2 AND rolled_back = 0",
        params![rollback_action_id, id],
    )?;
    if affected == 0 {
        must_get_by_id(conn, id)?;
        return Err(RepositoryError::BenchmarkRunAlreadyRolledBack(id));
    }
    must_get_by_id(conn, id)
}

/// Unit U10 (SRS FR-TUNE-003): whether at least one `benchmark_runs` row
/// exists whose action was of `action_type` and whose verdict was
/// `IMPROVED`, never rolled back — the real join `benchmark_runs` on its
/// own can't answer, since it only has `action_id`, not `action_type`
/// (that lives on `actions`, unit U7).
pub fn query_improved_run_exists(
    conn: &Connection,
    action_type: &str,
) -> Result<bool, RepositoryError> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM benchmark_runs
            JOIN actions ON benchmark_runs.action_id = actions.id
            WHERE actions.action_type = ?1
              AND benchmark_runs.verdict = 'IMPROVED'
              AND benchmark_runs.rolled_back = 0
         )",
        params![action_type],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count != 0)
    .map_err(Into::into)
}
