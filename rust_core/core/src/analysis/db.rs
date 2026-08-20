//! Database health classification (SRS FR-PERF-003). `classify_db` is pure
//! and synchronous — it consumes a `DbEvidenceBundle` the caller assembles
//! from one round of live `DbmsAdapter` calls, never talking to a database
//! itself. Evidence here is a single-poll snapshot, not a window: nothing
//! yet persists `dbms_snapshots` history to diff against (see
//! docs/adr/0010) — `Deadlocks`/`TempFileUsage` are evaluated as absolute
//! counts since `stats_reset`, not rates, for that reason.

use chrono::{DateTime, Utc};

use super::evidence::Evidence;
use super::thresholds::{self, Tier};
use crate::dbms::capability::Gated;
use crate::dbms::{
    DatabaseInfo, DeadlockInfo, GucValue, LockEdge, LongTransaction, QueryStat, ReplicationStatus,
    SessionInfo, SessionState, TableStat, TempFileActivity,
};

/// SRS FR-PERF-003's exact category list, plus `Availability` (whether the
/// poll that produced this bundle succeeded at all — see
/// [`unavailable_verdict`] for the failure case).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbHealthCategory {
    Availability,
    ConnectionSaturation,
    Latency,
    SlowQueries,
    LockWaits,
    Deadlocks,
    RollbackRatio,
    TempFileUsage,
    LongTransactions,
    ReplicationLag,
    BloatProxies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbHealthStatus {
    Ok,
    Warning,
    Critical,
    /// No evidence source for this category in this poll — never a faked
    /// value (matches the `Capability`/`Gated` model used throughout
    /// `core::dbms`).
    Unavailable,
}

fn status_for_tier(tier: Tier) -> DbHealthStatus {
    match tier {
        Tier::Normal | Tier::Elevated => DbHealthStatus::Ok,
        Tier::High => DbHealthStatus::Warning,
        Tier::Critical => DbHealthStatus::Critical,
    }
}

#[derive(Debug, Clone)]
pub struct DbHealthCheck {
    pub category: DbHealthCategory,
    pub status: DbHealthStatus,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone)]
pub struct DbHealthVerdict {
    pub checks: Vec<DbHealthCheck>,
    pub score: u8,
    pub score_version: &'static str,
}

/// Plain data assembled by the caller from one round of live `DbmsAdapter`
/// calls — `classify_db` itself never touches a connection.
#[derive(Debug, Clone)]
pub struct DbEvidenceBundle {
    pub databases: Vec<DatabaseInfo>,
    pub sessions: Vec<SessionInfo>,
    pub query_stats: Gated<Vec<QueryStat>>,
    pub locks: Vec<LockEdge>,
    pub table_stats: Vec<TableStat>,
    pub replication: ReplicationStatus,
    pub gucs: Vec<GucValue>,
    pub temp_file_activity: TempFileActivity,
    pub deadlocks: DeadlockInfo,
    pub long_transactions: Vec<LongTransaction>,
}

fn max_connections(gucs: &[GucValue]) -> Option<f64> {
    gucs.iter()
        .find(|g| g.name == "max_connections")
        .and_then(|g| g.setting.parse::<f64>().ok())
}

fn connection_saturation(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    match max_connections(&bundle.gucs) {
        Some(max) if max > 0.0 => {
            let ratio = bundle.sessions.len() as f64 / max;
            let tier = thresholds::classify_connection_saturation_tier(ratio);
            DbHealthCheck {
                category: DbHealthCategory::ConnectionSaturation,
                status: status_for_tier(tier),
                evidence: vec![Evidence::new(
                    "session_count_over_max_connections",
                    ratio,
                    thresholds::DB_CONN_SATURATION_HIGH,
                    None,
                    now,
                    now,
                )],
            }
        }
        _ => unavailable_check(DbHealthCategory::ConnectionSaturation),
    }
}

fn unavailable_check(category: DbHealthCategory) -> DbHealthCheck {
    DbHealthCheck {
        category,
        status: DbHealthStatus::Unavailable,
        evidence: Vec::new(),
    }
}

fn latency_and_slow_queries(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> [DbHealthCheck; 2] {
    match &bundle.query_stats {
        Gated::Supported(stats) => {
            let worst = stats
                .iter()
                .map(|s| s.mean_exec_time_ms)
                .fold(0.0_f64, f64::max);
            let slow_count = stats
                .iter()
                .filter(|s| s.mean_exec_time_ms >= thresholds::DB_LATENCY_MS_HIGH)
                .count();
            let latency_tier = thresholds::classify_latency_tier(worst);
            let slow_tier = thresholds::classify_slow_query_count_tier(slow_count as f64);
            [
                DbHealthCheck {
                    category: DbHealthCategory::Latency,
                    status: status_for_tier(latency_tier),
                    evidence: vec![Evidence::new(
                        "max_mean_exec_time_ms",
                        worst,
                        thresholds::DB_LATENCY_MS_HIGH,
                        Some("ms"),
                        now,
                        now,
                    )],
                },
                DbHealthCheck {
                    category: DbHealthCategory::SlowQueries,
                    status: status_for_tier(slow_tier),
                    evidence: vec![Evidence::new(
                        "slow_query_count",
                        slow_count as f64,
                        thresholds::DB_SLOW_QUERY_COUNT_HIGH,
                        None,
                        now,
                        now,
                    )],
                },
            ]
        }
        Gated::Limited { .. } | Gated::Unavailable { .. } | Gated::PermissionRequired { .. } => [
            unavailable_check(DbHealthCategory::Latency),
            unavailable_check(DbHealthCategory::SlowQueries),
        ],
    }
}

fn lock_waits(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    let waiting_sessions = bundle
        .sessions
        .iter()
        .filter(|s| s.state == SessionState::Waiting)
        .count();
    let count = bundle.locks.len().max(waiting_sessions);
    let tier = thresholds::classify_lock_wait_count_tier(count as f64);
    DbHealthCheck {
        category: DbHealthCategory::LockWaits,
        status: status_for_tier(tier),
        evidence: vec![Evidence::new(
            "blocking_lock_edges",
            count as f64,
            thresholds::DB_LOCK_WAIT_COUNT_HIGH,
            None,
            now,
            now,
        )],
    }
}

fn deadlocks(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    let count = bundle.deadlocks.deadlocks as f64;
    let tier = thresholds::classify_deadlock_count_tier(count);
    DbHealthCheck {
        category: DbHealthCategory::Deadlocks,
        status: status_for_tier(tier),
        evidence: vec![Evidence::new(
            "deadlocks_since_reset",
            count,
            thresholds::DB_DEADLOCK_COUNT_HIGH,
            None,
            bundle.deadlocks.stats_reset.unwrap_or(now),
            now,
        )],
    }
}

fn rollback_ratio(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    let commit: i64 = bundle.databases.iter().map(|d| d.xact_commit).sum();
    let rollback: i64 = bundle.databases.iter().map(|d| d.xact_rollback).sum();
    let total = commit + rollback;
    let ratio = if total > 0 {
        rollback as f64 / total as f64
    } else {
        0.0
    };
    let tier = thresholds::classify_rollback_ratio_tier(ratio);
    DbHealthCheck {
        category: DbHealthCategory::RollbackRatio,
        status: status_for_tier(tier),
        evidence: vec![Evidence::new(
            "rollback_ratio",
            ratio,
            thresholds::DB_ROLLBACK_RATIO_HIGH,
            None,
            now,
            now,
        )],
    }
}

fn temp_file_usage(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    let bytes = bundle.temp_file_activity.temp_bytes as f64;
    let tier = thresholds::classify_temp_bytes_tier(bytes);
    DbHealthCheck {
        category: DbHealthCategory::TempFileUsage,
        status: status_for_tier(tier),
        evidence: vec![Evidence::new(
            "temp_bytes_since_reset",
            bytes,
            thresholds::DB_TEMP_BYTES_HIGH,
            Some("bytes"),
            bundle.temp_file_activity.stats_reset.unwrap_or(now),
            now,
        )],
    }
}

fn long_transactions(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    let count = bundle.long_transactions.len() as f64;
    let tier = thresholds::classify_long_transaction_count_tier(count);
    DbHealthCheck {
        category: DbHealthCategory::LongTransactions,
        status: status_for_tier(tier),
        evidence: vec![Evidence::new(
            "long_transaction_count",
            count,
            thresholds::DB_LONG_TXN_COUNT_HIGH,
            None,
            now,
            now,
        )],
    }
}

fn replication_lag(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    if bundle.replication.standbys.is_empty() {
        return unavailable_check(DbHealthCategory::ReplicationLag);
    }
    let worst = bundle
        .replication
        .standbys
        .iter()
        .filter_map(|s| s.replay_lag_seconds)
        .fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        });
    match worst {
        Some(seconds) => {
            let tier = thresholds::classify_replication_lag_tier(seconds);
            DbHealthCheck {
                category: DbHealthCategory::ReplicationLag,
                status: status_for_tier(tier),
                evidence: vec![Evidence::new(
                    "max_replay_lag_seconds",
                    seconds,
                    thresholds::DB_REPLICATION_LAG_SECONDS_HIGH,
                    Some("s"),
                    now,
                    now,
                )],
            }
        }
        None => unavailable_check(DbHealthCategory::ReplicationLag),
    }
}

fn bloat_proxies(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthCheck {
    let worst = bundle
        .table_stats
        .iter()
        .filter(|t| t.n_live_tup + t.n_dead_tup >= thresholds::DB_BLOAT_MIN_ROWS)
        .filter_map(|t| {
            let total = t.n_live_tup + t.n_dead_tup;
            (total > 0).then_some(t.n_dead_tup as f64 / total as f64)
        })
        .fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        });
    match worst {
        Some(ratio) => {
            let tier = thresholds::classify_bloat_ratio_tier(ratio);
            DbHealthCheck {
                category: DbHealthCategory::BloatProxies,
                status: status_for_tier(tier),
                evidence: vec![Evidence::new(
                    "max_dead_tuple_ratio",
                    ratio,
                    thresholds::DB_BLOAT_RATIO_HIGH,
                    None,
                    now,
                    now,
                )],
            }
        }
        None => unavailable_check(DbHealthCategory::BloatProxies),
    }
}

/// Classifies database health from one already-successful poll. A failed
/// poll (connection refused, timeout, ...) never reaches this function —
/// use [`unavailable_verdict`] for that case instead, keeping the two
/// failure domains honestly distinct.
pub fn classify_db(bundle: &DbEvidenceBundle, now: DateTime<Utc>) -> DbHealthVerdict {
    let mut checks = vec![DbHealthCheck {
        category: DbHealthCategory::Availability,
        status: DbHealthStatus::Ok,
        evidence: vec![Evidence::new("poll_succeeded", 1.0, 1.0, None, now, now)],
    }];
    checks.push(connection_saturation(bundle, now));
    checks.extend(latency_and_slow_queries(bundle, now));
    checks.push(lock_waits(bundle, now));
    checks.push(deadlocks(bundle, now));
    checks.push(rollback_ratio(bundle, now));
    checks.push(temp_file_usage(bundle, now));
    checks.push(long_transactions(bundle, now));
    checks.push(replication_lag(bundle, now));
    checks.push(bloat_proxies(bundle, now));

    let score = super::score::score_db(&checks);

    DbHealthVerdict {
        checks,
        score,
        score_version: super::score::DB_SCORE_VERSION,
    }
}

/// The poll that would have fed `classify_db` failed outright — every
/// category is `Unavailable` except `Availability` itself, which is
/// `Critical`. Kept separate from `classify_db` rather than folded into it
/// with an `Option<DbEvidenceBundle>` parameter: a failed poll and a
/// successful-but-empty poll are different situations, and conflating them
/// would blur "we don't know" with "we checked and it's fine."
pub fn unavailable_verdict(reason: &str, now: DateTime<Utc>) -> DbHealthVerdict {
    let mut checks = vec![DbHealthCheck {
        category: DbHealthCategory::Availability,
        status: DbHealthStatus::Critical,
        evidence: vec![Evidence::new(reason, 0.0, 1.0, None, now, now)],
    }];
    for category in [
        DbHealthCategory::ConnectionSaturation,
        DbHealthCategory::Latency,
        DbHealthCategory::SlowQueries,
        DbHealthCategory::LockWaits,
        DbHealthCategory::Deadlocks,
        DbHealthCategory::RollbackRatio,
        DbHealthCategory::TempFileUsage,
        DbHealthCategory::LongTransactions,
        DbHealthCategory::ReplicationLag,
        DbHealthCategory::BloatProxies,
    ] {
        checks.push(unavailable_check(category));
    }

    let score = super::score::score_db(&checks);

    DbHealthVerdict {
        checks,
        score,
        score_version: super::score::DB_SCORE_VERSION,
    }
}
