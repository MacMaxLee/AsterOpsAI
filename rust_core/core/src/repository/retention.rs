//! Rollup, purge, and size-ceiling enforcement (requirement 5), driven by
//! `writer::WriteCommand::RunRetentionSweep`. Runs entirely on the writer
//! thread's own connection — no separate transaction/locking scheme needed
//! since there is exactly one writer.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};

use super::audit::insert_audit_event;
use super::error::RepositoryError;
use super::models::{NewAuditEvent, RetentionAuditDetail, RetentionReport};
use super::time::format_ts;

const RAW_RETENTION_DAYS: i64 = 7;
const HOURLY_RETENTION_DAYS: i64 = 90;
const DAILY_RETENTION_DAYS: i64 = 730;
/// A bucket still receiving live writes shouldn't be rolled up yet; this
/// grace period keeps the sweep from rolling up the current hour.
const HOURLY_ROLLUP_GRACE_MINUTES: i64 = 5;
/// Decimal (not binary) 500MB, the stricter of the two common readings of
/// "500MB" — triggers eviction sooner rather than later under any
/// interpretation of the requirement.
const SIZE_CEILING_BYTES: u64 = 500_000_000;
const EVICTION_BATCH_SIZE: i64 = 1000;
/// Backstop against a pathological loop; batches * this is far more rows
/// than the ceiling could ever actually contain at once.
const EVICTION_SAFETY_ITERATIONS: u32 = 10_000;

fn database_size_bytes(conn: &Connection) -> Result<u64, RepositoryError> {
    let page_count: i64 = conn.query_row("SELECT * FROM pragma_page_count()", [], |r| r.get(0))?;
    let page_size: i64 = conn.query_row("SELECT * FROM pragma_page_size()", [], |r| r.get(0))?;
    Ok((page_count.max(0) as u64) * (page_size.max(0) as u64))
}

fn purge_older_than(
    conn: &Connection,
    table: &str,
    column: &str,
    cutoff: DateTime<Utc>,
) -> Result<u64, RepositoryError> {
    let sql = format!("DELETE FROM {table} WHERE {column} < ?1");
    let deleted = conn.execute(&sql, params![format_ts(cutoff)])?;
    Ok(deleted as u64)
}

/// One `INSERT ... SELECT ... GROUP BY` per sweep, covering every
/// fully-elapsed hour not yet present in `telemetry_rollup_hourly`.
/// avg/min/max are computed only over `SUPPORTED`-state samples (via `CASE
/// WHEN`, ignored like `FILTER (WHERE ...)` would be); `*_count` columns
/// make the supported/total mix visible to readers rather than ever
/// fabricating an average over gap/unavailable samples. `INSERT OR IGNORE`
/// (the tables' `UNIQUE (bucket_start)`) makes this naturally idempotent
/// across repeated sweeps.
fn compute_hourly_rollups(conn: &Connection, now: DateTime<Utc>) -> Result<u64, RepositoryError> {
    let cutoff = now - Duration::minutes(HOURLY_ROLLUP_GRACE_MINUTES);
    let sql = "
        INSERT OR IGNORE INTO telemetry_rollup_hourly (
            bucket_start, bucket_end,
            cpu_util_avg, cpu_util_min, cpu_util_max, cpu_util_supported_count, cpu_util_total_count,
            mem_used_avg, mem_used_min, mem_used_max, mem_used_supported_count, mem_used_total_count,
            storage_read_bps_avg, storage_write_bps_avg, net_rx_bps_avg, net_tx_bps_avg
        )
        SELECT
            strftime('%Y-%m-%dT%H:00:00.000Z', ts) AS bucket_start,
            MIN(strftime('%Y-%m-%dT%H:00:00.000Z', ts, '+1 hour')) AS bucket_end,
            AVG(CASE WHEN cpu_aggregate_util_state = 'SUPPORTED' THEN cpu_aggregate_util_pct END),
            MIN(CASE WHEN cpu_aggregate_util_state = 'SUPPORTED' THEN cpu_aggregate_util_pct END),
            MAX(CASE WHEN cpu_aggregate_util_state = 'SUPPORTED' THEN cpu_aggregate_util_pct END),
            SUM(CASE WHEN cpu_aggregate_util_state = 'SUPPORTED' THEN 1 ELSE 0 END),
            COUNT(*),
            AVG(CASE WHEN mem_used_bytes_state = 'SUPPORTED' THEN mem_used_bytes END),
            MIN(CASE WHEN mem_used_bytes_state = 'SUPPORTED' THEN mem_used_bytes END),
            MAX(CASE WHEN mem_used_bytes_state = 'SUPPORTED' THEN mem_used_bytes END),
            SUM(CASE WHEN mem_used_bytes_state = 'SUPPORTED' THEN 1 ELSE 0 END),
            COUNT(*),
            AVG(storage_read_bytes_ps), AVG(storage_write_bytes_ps),
            AVG(net_rx_bytes_ps), AVG(net_tx_bytes_ps)
        FROM telemetry_snapshots
        WHERE ts < ?1
        GROUP BY bucket_start
    ";
    let inserted = conn.execute(sql, params![format_ts(cutoff)])?;
    Ok(inserted as u64)
}

/// Aggregated from `telemetry_rollup_hourly` (avg-of-avgs, min-of-mins,
/// max-of-maxes) — not a raw re-scan, since raw for that day may already be
/// purged by the time this runs. Only rolls up a day once all 24 hourly
/// rows exist (`HAVING COUNT(*) = 24`), so a partially-elapsed day is never
/// rolled up early.
fn compute_daily_rollups(conn: &Connection) -> Result<u64, RepositoryError> {
    let sql = "
        INSERT OR IGNORE INTO telemetry_rollup_daily (
            bucket_start, bucket_end,
            cpu_util_avg, cpu_util_min, cpu_util_max, cpu_util_supported_count, cpu_util_total_count,
            mem_used_avg, mem_used_min, mem_used_max, mem_used_supported_count, mem_used_total_count,
            storage_read_bps_avg, storage_write_bps_avg, net_rx_bps_avg, net_tx_bps_avg
        )
        SELECT
            date(bucket_start) || 'T00:00:00.000Z' AS day_start,
            MIN(strftime('%Y-%m-%dT00:00:00.000Z', bucket_start, '+1 day')) AS day_end,
            AVG(cpu_util_avg), MIN(cpu_util_min), MAX(cpu_util_max),
            SUM(cpu_util_supported_count), SUM(cpu_util_total_count),
            AVG(mem_used_avg), MIN(mem_used_min), MAX(mem_used_max),
            SUM(mem_used_supported_count), SUM(mem_used_total_count),
            AVG(storage_read_bps_avg), AVG(storage_write_bps_avg),
            AVG(net_rx_bps_avg), AVG(net_tx_bps_avg)
        FROM telemetry_rollup_hourly
        GROUP BY day_start
        HAVING COUNT(*) = 24
    ";
    let inserted = conn.execute(sql, [])?;
    Ok(inserted as u64)
}

#[derive(Debug, Clone, Copy)]
enum TelemetryTable {
    Raw,
    Hourly,
    Daily,
}

impl TelemetryTable {
    fn name(self) -> &'static str {
        match self {
            Self::Raw => "telemetry_snapshots",
            Self::Hourly => "telemetry_rollup_hourly",
            Self::Daily => "telemetry_rollup_daily",
        }
    }

    fn order_column(self) -> &'static str {
        match self {
            Self::Raw => "ts",
            Self::Hourly | Self::Daily => "bucket_start",
        }
    }
}

/// Finds which of the three telemetry tables currently holds the
/// globally-oldest row, by comparing `MIN(ts)`/`MIN(bucket_start)` across
/// all three. Never considers `audit_events`/`actions`/
/// `performance_analysis`/`security_*` — those are low-volume and, in
/// `audit_events`' case specifically, must never be pruned by size
/// pressure (deleting a link invalidates every hash after it).
fn find_oldest_telemetry_table(
    conn: &Connection,
) -> Result<Option<TelemetryTable>, RepositoryError> {
    let mut candidates: Vec<(String, TelemetryTable)> = Vec::new();
    for table in [
        TelemetryTable::Raw,
        TelemetryTable::Hourly,
        TelemetryTable::Daily,
    ] {
        let sql = format!("SELECT MIN({}) FROM {}", table.order_column(), table.name());
        let min: Option<String> = conn.query_row(&sql, [], |r| r.get(0))?;
        if let Some(min) = min {
            candidates.push((min, table));
        }
    }
    Ok(candidates
        .into_iter()
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, t)| t))
}

fn evict_batch(
    conn: &Connection,
    table: TelemetryTable,
    limit: i64,
) -> Result<u64, RepositoryError> {
    let sql = format!(
        "DELETE FROM {name} WHERE rowid IN (SELECT rowid FROM {name} ORDER BY {col} ASC LIMIT ?1)",
        name = table.name(),
        col = table.order_column(),
    );
    let deleted = conn.execute(&sql, params![limit])?;
    Ok(deleted as u64)
}

#[derive(Debug, Default)]
struct EvictedCounts {
    raw: u64,
    hourly: u64,
    daily: u64,
}

/// Deletes globally-oldest telemetry rows, batch by batch, running
/// `incremental_vacuum` between batches so each size re-check reflects
/// genuinely reclaimed space, until the database is back under the ceiling
/// or the safety-iteration cap is hit.
fn enforce_size_ceiling(conn: &Connection) -> Result<(EvictedCounts, bool), RepositoryError> {
    let mut evicted = EvictedCounts::default();
    let mut breached = false;
    let mut size = database_size_bytes(conn)?;
    let mut iterations = 0;

    while size > SIZE_CEILING_BYTES && iterations < EVICTION_SAFETY_ITERATIONS {
        breached = true;
        iterations += 1;

        match find_oldest_telemetry_table(conn)? {
            Some(TelemetryTable::Raw) => {
                evicted.raw += evict_batch(conn, TelemetryTable::Raw, EVICTION_BATCH_SIZE)?;
            }
            Some(TelemetryTable::Hourly) => {
                evicted.hourly += evict_batch(conn, TelemetryTable::Hourly, EVICTION_BATCH_SIZE)?;
            }
            Some(TelemetryTable::Daily) => {
                evicted.daily += evict_batch(conn, TelemetryTable::Daily, EVICTION_BATCH_SIZE)?;
            }
            None => break, // nothing left to evict from any telemetry table
        }

        conn.execute("PRAGMA incremental_vacuum(2000)", [])?;
        size = database_size_bytes(conn)?;
    }

    Ok((evicted, breached))
}

/// Runs one full sweep: rollup, age-based purge, size-ceiling eviction,
/// then records its own audit event through the same hash-chained path as
/// any other. `next_audit_id`/`last_row_hash` are the writer thread's local
/// chain state, threaded through by `&mut` reference and updated in place.
pub fn run_sweep(
    conn: &Connection,
    next_audit_id: &mut i64,
    last_row_hash: &mut String,
    now: DateTime<Utc>,
) -> Result<RetentionReport, RepositoryError> {
    let db_size_before_bytes = database_size_bytes(conn)?;

    let hourly_rollups_computed = compute_hourly_rollups(conn, now)?;
    let daily_rollups_computed = compute_daily_rollups(conn)?;

    let mut rows_deleted_raw = purge_older_than(
        conn,
        "telemetry_snapshots",
        "ts",
        now - Duration::days(RAW_RETENTION_DAYS),
    )?;
    let mut rows_deleted_hourly = purge_older_than(
        conn,
        "telemetry_rollup_hourly",
        "bucket_end",
        now - Duration::days(HOURLY_RETENTION_DAYS),
    )?;
    let mut rows_deleted_daily = purge_older_than(
        conn,
        "telemetry_rollup_daily",
        "bucket_end",
        now - Duration::days(DAILY_RETENTION_DAYS),
    )?;

    let (evicted, ceiling_breached) = enforce_size_ceiling(conn)?;
    rows_deleted_raw += evicted.raw;
    rows_deleted_hourly += evicted.hourly;
    rows_deleted_daily += evicted.daily;

    let db_size_after_bytes = database_size_bytes(conn)?;

    let detail = RetentionAuditDetail {
        rows_deleted_raw,
        rows_deleted_hourly,
        rows_deleted_daily,
        hourly_rollups_computed,
        daily_rollups_computed,
        db_size_before_bytes,
        db_size_after_bytes,
        ceiling_bytes: SIZE_CEILING_BYTES,
        ceiling_breached,
    };

    let summary = format!(
        "purged {rows_deleted_raw} raw / {rows_deleted_hourly} hourly / {rows_deleted_daily} daily rows; \
         computed {hourly_rollups_computed} hourly / {daily_rollups_computed} daily rollups; \
         size {db_size_before_bytes} -> {db_size_after_bytes} bytes (ceiling {SIZE_CEILING_BYTES}, breached: {ceiling_breached})"
    );
    let new_event = NewAuditEvent {
        ts: now,
        event_type: "RETENTION_PURGE".to_string(),
        actor: "system:retention-job".to_string(),
        summary,
        detail_json: serde_json::to_string(&detail)?,
    };

    let id = *next_audit_id;
    let recorded = insert_audit_event(conn, id, last_row_hash, &new_event)?;
    *next_audit_id += 1;
    *last_row_hash = recorded.row_hash.clone();

    Ok(RetentionReport {
        detail,
        audit_row: Some(recorded),
    })
}
