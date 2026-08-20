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

pub fn mark_rolled_back(
    conn: &Connection,
    id: i64,
    rollback_action_id: i64,
) -> Result<BenchmarkRunRow, RepositoryError> {
    conn.execute(
        "UPDATE benchmark_runs SET rolled_back = 1, rollback_action_id = ?1 WHERE id = ?2",
        params![rollback_action_id, id],
    )?;
    must_get_by_id(conn, id)
}
