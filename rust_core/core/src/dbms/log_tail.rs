//! Unit U60 (SRS FR-DBSEC-001(a)): reads new rows from a real,
//! locally-readable PostgreSQL CSV server log — the one I/O class
//! every other `core::dbms` capability doesn't need, since every
//! other trait method is a pure SQL query. Deliberately **not** a
//! `DbmsAdapter` trait method: unlike a SQL query, this is
//! fundamentally a local-host filesystem operation a genuinely
//! network-only adapter could never honestly implement, and adding it
//! to the trait would silently imply every future adapter must
//! support local file access. `service` calls this directly,
//! alongside (not through) the adapter — see docs/adr/0065 for the
//! full scope decision (co-located deployments only).
//!
//! Deduplication is the byte-offset mechanism itself, not detector
//! logic: each call reads only the bytes appended since the last
//! persisted offset (`repository::log_tail_offset`), so a given log
//! line is only ever parsed once — see `security::detect_auth_
//! failure`'s own doc comment for why it has no fire/no-fire branch.

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// PostgreSQL's own documented SQLSTATE codes for a connection that
/// never got in: `28P01` (invalid password) and `28000` (invalid
/// authorization specification, e.g. no matching `pg_hba.conf` line).
/// Deliberately not free-text `message` matching — SQLSTATE is
/// PostgreSQL's own stable, version-independent signal.
const AUTH_FAILURE_SQLSTATES: &[&str] = &["28P01", "28000"];

/// Column indices in PostgreSQL's own `csvlog` format (confirmed
/// empirically against a real PG17 instance; stable across versions
/// per PostgreSQL's own documented log format).
const COL_LOG_TIME: usize = 0;
const COL_USER_NAME: usize = 1;
const COL_DATABASE_NAME: usize = 2;
const COL_CONNECTION_FROM: usize = 4;
const COL_SQL_STATE_CODE: usize = 12;
const COL_MESSAGE: usize = 13;

#[derive(Debug, Clone, PartialEq)]
pub struct AuthFailureEvent {
    pub user_name: Option<String>,
    pub database_name: Option<String>,
    pub connection_from: Option<String>,
    pub message: String,
    pub log_time: DateTime<Utc>,
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// PostgreSQL's own `log_time` format is `YYYY-MM-DD HH:MM:SS.mmm
/// TZD` (e.g. `2026-08-19 12:34:56.789 UTC`) — the trailing timezone
/// abbreviation is dropped (this detector's scope is a co-located
/// deployment; `log_timezone` is assumed UTC or otherwise
/// irrelevant to the second-level ordering this timestamp is used
/// for) rather than parsed, mirroring `repository::time::parse_ts`'s
/// own "fall back rather than error on a malformed timestamp"
/// precedent.
fn parse_log_time(text: &str) -> DateTime<Utc> {
    let naive_part = text.rsplit_once(' ').map_or(text, |(naive, _tz)| naive);
    NaiveDateTime::parse_from_str(naive_part, "%Y-%m-%d %H:%M:%S%.f")
        .map(|naive| naive.and_utc())
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

fn parse_auth_failure_row(record: &csv::StringRecord) -> Option<AuthFailureEvent> {
    let sql_state_code = record.get(COL_SQL_STATE_CODE)?;
    if !AUTH_FAILURE_SQLSTATES.contains(&sql_state_code) {
        return None;
    }
    Some(AuthFailureEvent {
        user_name: record.get(COL_USER_NAME).and_then(non_empty),
        database_name: record.get(COL_DATABASE_NAME).and_then(non_empty),
        connection_from: record.get(COL_CONNECTION_FROM).and_then(non_empty),
        message: record.get(COL_MESSAGE).unwrap_or_default().to_string(),
        log_time: record.get(COL_LOG_TIME).map_or(Utc::now(), parse_log_time),
    })
}

/// PostgreSQL rotates its own log files; "most recently modified
/// `*.csv` file in `log_dir`" is the real, currently-active one.
/// `pub`, not just an internal helper of [`read_new_auth_failures`]:
/// the caller (`service`) needs the active file's own path *before*
/// it can look up that file's previously-persisted offset
/// (`repository::log_tail_offset`, keyed by file path) to pass back
/// in as `since_offset`.
pub async fn find_active_csv_file(log_dir: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut entries = tokio::fs::read_dir(log_dir).await?;
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("csv") {
            continue;
        }
        let modified = entry.metadata().await?.modified()?;
        if best
            .as_ref()
            .is_none_or(|(_, best_modified)| modified > *best_modified)
        {
            best = Some((path, modified));
        }
    }
    Ok(best.map(|(path, _)| path))
}

/// Reads every new, complete auth-failure log row since `since_offset`
/// (`None` means "never tailed before" — starts at byte 0; a
/// `Some((path, offset))` whose `path` no longer matches the
/// currently-active file, e.g. after log rotation, also starts at
/// byte 0 rather than reusing a stale offset into a now-inactive
/// file). A trailing partial record (the file is still being written
/// to) is left unconsumed — the returned offset only ever advances
/// past whole, successfully parsed records, so it's picked up whole
/// on the next tail. Returns `(events, active_file_path,
/// new_byte_offset)`; an empty `log_dir` (no `*.csv` file yet)
/// returns an empty event list and offset 0 against `log_dir` itself.
pub async fn read_new_auth_failures(
    log_dir: &Path,
    since_offset: Option<(PathBuf, i64)>,
) -> std::io::Result<(Vec<AuthFailureEvent>, PathBuf, i64)> {
    let Some(active_file) = find_active_csv_file(log_dir).await? else {
        return Ok((Vec::new(), log_dir.to_path_buf(), 0));
    };

    let start_offset = match &since_offset {
        Some((path, offset)) if *path == active_file => *offset as u64,
        _ => 0,
    };

    let mut file = tokio::fs::File::open(&active_file).await?;
    file.seek(std::io::SeekFrom::Start(start_offset)).await?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(buf.as_slice());

    let mut events = Vec::new();
    let mut consumed: u64 = 0;
    let mut record = csv::StringRecord::new();
    loop {
        match reader.read_record(&mut record) {
            Ok(true) => {
                if let Some(event) = parse_auth_failure_row(&record) {
                    events.push(event);
                }
                consumed = reader.position().byte();
            }
            Ok(false) => break,
            // A partial trailing record (concurrent writer hasn't
            // finished this line yet) — stop here; `consumed` still
            // reflects only whole records already parsed, so the
            // partial line is retried whole on the next tail.
            Err(_) => break,
        }
    }

    Ok((events, active_file, start_offset as i64 + consumed as i64))
}
