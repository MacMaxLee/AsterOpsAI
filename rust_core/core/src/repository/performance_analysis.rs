use rusqlite::{params, Connection};

use super::error::RepositoryError;
use super::models::PerformanceAnalysisRow;
use super::time::format_ts;

/// Inserts one performance-analysis result. The mapping from `analysis::
/// HostVerdict`/`DbHealthVerdict` to this row (JSON-encoding the evidence
/// list, choosing `subject`) lives in whatever later unit wires up a live
/// poll loop — this crate only knows about the row shape, matching
/// `telemetry_store::insert_raw_snapshot`'s own precedent.
pub fn insert_performance_analysis(
    conn: &Connection,
    row: &PerformanceAnalysisRow,
) -> Result<(), RepositoryError> {
    conn.execute(
        "INSERT INTO performance_analysis (ts, subject, score, score_version, verdict, evidence_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            format_ts(row.ts),
            row.subject,
            row.score,
            row.score_version,
            row.verdict,
            row.evidence_json,
        ],
    )?;
    Ok(())
}
