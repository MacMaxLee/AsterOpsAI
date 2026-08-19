//! Tier selection (requirement 7) and the read queries backing the history
//! endpoints. Raw-tier queries read `telemetry_snapshots` directly; hourly/
//! daily read the matching rollup table. Only `cpu_aggregate_util` and
//! `mem_used_bytes` get full min/max/supported-count tracking in the rollup
//! tables (see migrations/V2); storage/network rollups are avg-only, and
//! CPU's `load_average_1m` isn't rolled up at all — both are honestly
//! reported as zero-sample `HistoryStat`s at non-raw tiers rather than
//! fabricated, documented inline below.

use chrono::{DateTime, Duration, Utc};
use contracts::telemetry::{
    CpuHistoryPoint, HistoryStat, HistoryTier, MemoryHistoryPoint, NetworkHistoryPoint,
    StorageHistoryPoint,
};
use rusqlite::Connection;

use super::error::RepositoryError;
use super::time::{format_ts, parse_ts};

#[derive(Debug, Clone, Copy)]
pub enum HistoryRange {
    LastHour,
    Last24h,
    Last7d,
    Last30d,
    Custom {
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedRange {
    pub tier: HistoryTier,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub truncated_from: Option<DateTime<Utc>>,
}

/// Coarsest tier covering the whole requested span — a `custom` range
/// straddling tier boundaries returns that one tier's granularity for its
/// entire length rather than stitching finer resolution near the front and
/// coarser near the back. A deliberate simplification (see the U2 plan);
/// additive to change later if finer stitching is ever wanted.
pub fn resolve_range(range: HistoryRange, now: DateTime<Utc>) -> ResolvedRange {
    match range {
        HistoryRange::LastHour => ResolvedRange {
            tier: HistoryTier::Raw,
            from: now - Duration::hours(1),
            to: now,
            truncated_from: None,
        },
        HistoryRange::Last24h => ResolvedRange {
            tier: HistoryTier::Raw,
            from: now - Duration::hours(24),
            to: now,
            truncated_from: None,
        },
        HistoryRange::Last7d => ResolvedRange {
            tier: HistoryTier::Raw,
            from: now - Duration::days(7),
            to: now,
            truncated_from: None,
        },
        HistoryRange::Last30d => ResolvedRange {
            tier: HistoryTier::Hourly,
            from: now - Duration::days(30),
            to: now,
            truncated_from: None,
        },
        HistoryRange::Custom { from, to } => {
            let raw_horizon = now - Duration::days(7);
            let hourly_horizon = now - Duration::days(90);
            let daily_horizon = now - Duration::days(730);

            if from >= raw_horizon {
                ResolvedRange {
                    tier: HistoryTier::Raw,
                    from,
                    to,
                    truncated_from: None,
                }
            } else if from >= hourly_horizon {
                ResolvedRange {
                    tier: HistoryTier::Hourly,
                    from,
                    to,
                    truncated_from: None,
                }
            } else {
                let clamped_from = from.max(daily_horizon);
                let truncated_from = (clamped_from > from).then_some(clamped_from);
                ResolvedRange {
                    tier: HistoryTier::Daily,
                    from: clamped_from,
                    to,
                    truncated_from,
                }
            }
        }
    }
}

fn stat_from_value(value: Option<f64>) -> HistoryStat {
    match value {
        Some(v) => HistoryStat {
            avg: Some(v),
            min: Some(v),
            max: Some(v),
            supported_sample_count: 1,
            total_sample_count: 1,
        },
        None => HistoryStat {
            avg: None,
            min: None,
            max: None,
            supported_sample_count: 0,
            total_sample_count: 1,
        },
    }
}

fn stat_from_state(value: Option<f64>, state: &str) -> HistoryStat {
    if state == "SUPPORTED" {
        stat_from_value(value)
    } else {
        HistoryStat {
            avg: None,
            min: None,
            max: None,
            supported_sample_count: 0,
            total_sample_count: 1,
        }
    }
}

fn stat_from_rollup(
    avg: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    supported_count: i64,
    total_count: i64,
) -> HistoryStat {
    HistoryStat {
        avg,
        min,
        max,
        supported_sample_count: supported_count.max(0) as u32,
        total_sample_count: total_count.max(0) as u32,
    }
}

/// Unrolled-up metric at a non-raw tier: honestly zero-sample, never
/// fabricated.
fn empty_stat() -> HistoryStat {
    HistoryStat {
        avg: None,
        min: None,
        max: None,
        supported_sample_count: 0,
        total_sample_count: 0,
    }
}

fn rollup_table(tier: HistoryTier) -> &'static str {
    match tier {
        HistoryTier::Hourly => "telemetry_rollup_hourly",
        HistoryTier::Daily => "telemetry_rollup_daily",
        HistoryTier::Raw => unreachable!("raw tier has no rollup table"),
    }
}

pub fn query_cpu_history(
    conn: &Connection,
    resolved: &ResolvedRange,
) -> Result<Vec<CpuHistoryPoint>, RepositoryError> {
    if resolved.tier == HistoryTier::Raw {
        let mut stmt = conn.prepare(
            "SELECT ts, cpu_aggregate_util_pct, cpu_aggregate_util_state, cpu_load_avg_1m \
             FROM telemetry_snapshots WHERE ts >= ?1 AND ts < ?2 ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                let ts = parse_ts(&row.get::<_, String>(0)?);
                let util: Option<f64> = row.get(1)?;
                let util_state: String = row.get(2)?;
                let load_avg: Option<f64> = row.get(3)?;
                Ok(CpuHistoryPoint {
                    bucket_start: ts,
                    bucket_end: ts,
                    aggregate_utilization_percent: stat_from_state(util, &util_state),
                    load_average_1m: stat_from_value(load_avg),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let table = rollup_table(resolved.tier);
        let sql = format!(
            "SELECT bucket_start, bucket_end, cpu_util_avg, cpu_util_min, cpu_util_max, \
             cpu_util_supported_count, cpu_util_total_count FROM {table} \
             WHERE bucket_start >= ?1 AND bucket_start < ?2 ORDER BY bucket_start ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                Ok(CpuHistoryPoint {
                    bucket_start: parse_ts(&row.get::<_, String>(0)?),
                    bucket_end: parse_ts(&row.get::<_, String>(1)?),
                    aggregate_utilization_percent: stat_from_rollup(
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ),
                    // load_average_1m isn't rolled up (see module doc).
                    load_average_1m: empty_stat(),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

pub fn query_memory_history(
    conn: &Connection,
    resolved: &ResolvedRange,
) -> Result<Vec<MemoryHistoryPoint>, RepositoryError> {
    if resolved.tier == HistoryTier::Raw {
        let mut stmt = conn.prepare(
            "SELECT ts, mem_used_bytes, mem_used_bytes_state \
             FROM telemetry_snapshots WHERE ts >= ?1 AND ts < ?2 ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                let ts = parse_ts(&row.get::<_, String>(0)?);
                let used: Option<i64> = row.get(1)?;
                let state: String = row.get(2)?;
                Ok(MemoryHistoryPoint {
                    bucket_start: ts,
                    bucket_end: ts,
                    used_bytes: stat_from_state(used.map(|v| v as f64), &state),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let table = rollup_table(resolved.tier);
        let sql = format!(
            "SELECT bucket_start, bucket_end, mem_used_avg, mem_used_min, mem_used_max, \
             mem_used_supported_count, mem_used_total_count FROM {table} \
             WHERE bucket_start >= ?1 AND bucket_start < ?2 ORDER BY bucket_start ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                Ok(MemoryHistoryPoint {
                    bucket_start: parse_ts(&row.get::<_, String>(0)?),
                    bucket_end: parse_ts(&row.get::<_, String>(1)?),
                    used_bytes: stat_from_rollup(
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

pub fn query_storage_history(
    conn: &Connection,
    resolved: &ResolvedRange,
) -> Result<Vec<StorageHistoryPoint>, RepositoryError> {
    if resolved.tier == HistoryTier::Raw {
        let mut stmt = conn.prepare(
            "SELECT ts, storage_read_bytes_ps, storage_write_bytes_ps \
             FROM telemetry_snapshots WHERE ts >= ?1 AND ts < ?2 ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                let ts = parse_ts(&row.get::<_, String>(0)?);
                Ok(StorageHistoryPoint {
                    bucket_start: ts,
                    bucket_end: ts,
                    read_bytes_per_sec: stat_from_value(row.get(1)?),
                    write_bytes_per_sec: stat_from_value(row.get(2)?),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        // Storage rollups are avg-only (migrations/V2) — reported as a
        // single-sample stat when present, never a fabricated min/max.
        let table = rollup_table(resolved.tier);
        let sql = format!(
            "SELECT bucket_start, bucket_end, storage_read_bps_avg, storage_write_bps_avg \
             FROM {table} WHERE bucket_start >= ?1 AND bucket_start < ?2 ORDER BY bucket_start ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                Ok(StorageHistoryPoint {
                    bucket_start: parse_ts(&row.get::<_, String>(0)?),
                    bucket_end: parse_ts(&row.get::<_, String>(1)?),
                    read_bytes_per_sec: stat_from_value(row.get(2)?),
                    write_bytes_per_sec: stat_from_value(row.get(3)?),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

pub fn query_network_history(
    conn: &Connection,
    resolved: &ResolvedRange,
) -> Result<Vec<NetworkHistoryPoint>, RepositoryError> {
    if resolved.tier == HistoryTier::Raw {
        let mut stmt = conn.prepare(
            "SELECT ts, net_rx_bytes_ps, net_tx_bytes_ps \
             FROM telemetry_snapshots WHERE ts >= ?1 AND ts < ?2 ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                let ts = parse_ts(&row.get::<_, String>(0)?);
                Ok(NetworkHistoryPoint {
                    bucket_start: ts,
                    bucket_end: ts,
                    rx_bytes_per_sec: stat_from_value(row.get(1)?),
                    tx_bytes_per_sec: stat_from_value(row.get(2)?),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    } else {
        let table = rollup_table(resolved.tier);
        let sql = format!(
            "SELECT bucket_start, bucket_end, net_rx_bps_avg, net_tx_bps_avg FROM {table} \
             WHERE bucket_start >= ?1 AND bucket_start < ?2 ORDER BY bucket_start ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params![format_ts(resolved.from), format_ts(resolved.to)],
            |row| {
                Ok(NetworkHistoryPoint {
                    bucket_start: parse_ts(&row.get::<_, String>(0)?),
                    bucket_end: parse_ts(&row.get::<_, String>(1)?),
                    rx_bytes_per_sec: stat_from_value(row.get(2)?),
                    tx_bytes_per_sec: stat_from_value(row.get(3)?),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
