//! Fixture-based coverage of `analysis::classify_db` (SRS FR-PERF-003).
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::analysis::{
    classify_db, unavailable_verdict, DbEvidenceBundle, DbHealthCategory, DbHealthStatus,
};
use ai_ops_core::dbms::capability::Gated;
use ai_ops_core::dbms::{
    DatabaseInfo, DeadlockInfo, GucValue, LockEdge, LongTransaction, QueryStat, ReplicationStatus,
    SessionInfo, SessionState, StandbyInfo, TableStat, TempFileActivity,
};
use chrono::{DateTime, Utc};

fn now() -> DateTime<Utc> {
    "2026-01-01T00:00:00.000Z".parse().unwrap()
}

fn empty_bundle() -> DbEvidenceBundle {
    DbEvidenceBundle {
        databases: vec![DatabaseInfo {
            name: "app".to_string(),
            size_bytes: 1_000_000,
            xact_commit: 1000,
            xact_rollback: 5,
        }],
        sessions: Vec::new(),
        query_stats: Gated::Unavailable {
            reason: "pg_stat_statements not loaded".to_string(),
        },
        locks: Vec::new(),
        table_stats: Vec::new(),
        replication: ReplicationStatus {
            is_primary: true,
            in_recovery: false,
            standbys: Vec::new(),
        },
        gucs: vec![GucValue {
            name: "max_connections".to_string(),
            setting: "100".to_string(),
            unit: None,
            source: "configuration file".to_string(),
        }],
        temp_file_activity: TempFileActivity {
            temp_files: 0,
            temp_bytes: 0,
            stats_reset: Some(now()),
        },
        deadlocks: DeadlockInfo {
            deadlocks: 0,
            stats_reset: Some(now()),
        },
        long_transactions: Vec::new(),
    }
}

fn status_of(
    verdict: &ai_ops_core::analysis::DbHealthVerdict,
    category: DbHealthCategory,
) -> DbHealthStatus {
    verdict
        .checks
        .iter()
        .find(|c| c.category == category)
        .expect("category present")
        .status
}

#[test]
fn clean_bundle_is_all_ok_or_unavailable() {
    let verdict = classify_db(&empty_bundle(), now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::Availability),
        DbHealthStatus::Ok
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ConnectionSaturation),
        DbHealthStatus::Ok
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::Latency),
        DbHealthStatus::Unavailable
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::SlowQueries),
        DbHealthStatus::Unavailable
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ReplicationLag),
        DbHealthStatus::Unavailable
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::BloatProxies),
        DbHealthStatus::Unavailable
    );
    assert_eq!(verdict.score, 100);
}

#[test]
fn connection_saturation_critical_when_near_max() {
    let mut bundle = empty_bundle();
    bundle.sessions = (0..96).map(session).collect();
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ConnectionSaturation),
        DbHealthStatus::Critical
    );
}

fn session(pid: i32) -> SessionInfo {
    SessionInfo {
        pid,
        username: Some("app".to_string()),
        database: Some("app".to_string()),
        state: SessionState::Idle,
        client_addr: None,
        xact_start: None,
        query_start: None,
        query: None,
    }
}

#[test]
fn missing_max_connections_guc_is_unavailable_not_zero() {
    let mut bundle = empty_bundle();
    bundle.gucs.clear();
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ConnectionSaturation),
        DbHealthStatus::Unavailable
    );
}

#[test]
fn slow_queries_supported_query_stats_classified() {
    let mut bundle = empty_bundle();
    bundle.query_stats = Gated::Supported(vec![QueryStat {
        query_fingerprint: "abc".to_string(),
        normalized_query: "SELECT $1".to_string(),
        calls: 10,
        total_exec_time_ms: 30_000.0,
        mean_exec_time_ms: 3000.0,
        rows: 10,
    }]);
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::Latency),
        DbHealthStatus::Critical
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::SlowQueries),
        DbHealthStatus::Ok
    );
}

#[test]
fn lock_waits_from_lock_graph() {
    let mut bundle = empty_bundle();
    bundle.locks = (0..6)
        .map(|i| LockEdge {
            blocked_pid: i,
            blocked_query: None,
            blocking_pid: 1,
            blocking_query: None,
            lock_type: "ExclusiveLock".to_string(),
        })
        .collect();
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::LockWaits),
        DbHealthStatus::Warning
    );
}

#[test]
fn deadlocks_since_reset_classified() {
    let mut bundle = empty_bundle();
    bundle.deadlocks.deadlocks = 5;
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::Deadlocks),
        DbHealthStatus::Warning
    );
}

#[test]
fn rollback_ratio_high_when_many_rollbacks() {
    let mut bundle = empty_bundle();
    bundle.databases = vec![DatabaseInfo {
        name: "app".to_string(),
        size_bytes: 0,
        xact_commit: 80,
        xact_rollback: 20,
    }];
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::RollbackRatio),
        DbHealthStatus::Critical
    );
}

#[test]
fn rollback_ratio_with_no_transactions_is_ok_not_nan() {
    let mut bundle = empty_bundle();
    bundle.databases = vec![DatabaseInfo {
        name: "app".to_string(),
        size_bytes: 0,
        xact_commit: 0,
        xact_rollback: 0,
    }];
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::RollbackRatio),
        DbHealthStatus::Ok
    );
}

#[test]
fn temp_file_usage_critical_at_10gib() {
    let mut bundle = empty_bundle();
    bundle.temp_file_activity.temp_bytes = 11 * 1024 * 1024 * 1024;
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::TempFileUsage),
        DbHealthStatus::Critical
    );
}

#[test]
fn long_transactions_counted() {
    let mut bundle = empty_bundle();
    bundle.long_transactions = vec![LongTransaction {
        pid: 1,
        username: Some("app".to_string()),
        duration_seconds: 600.0,
        state: SessionState::Active,
        query: None,
    }];
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::LongTransactions),
        DbHealthStatus::Ok
    );
}

#[test]
fn replication_lag_from_standby() {
    let mut bundle = empty_bundle();
    bundle.replication.standbys = vec![StandbyInfo {
        client_addr: Some("10.0.0.2".to_string()),
        state: "streaming".to_string(),
        sent_lsn: Some("0/100".to_string()),
        write_lsn: Some("0/100".to_string()),
        flush_lsn: Some("0/100".to_string()),
        replay_lsn: Some("0/0F0".to_string()),
        replay_lag_seconds: Some(200.0),
    }];
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ReplicationLag),
        DbHealthStatus::Critical
    );
}

#[test]
fn no_standbys_is_unavailable_not_ok() {
    let bundle = empty_bundle();
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ReplicationLag),
        DbHealthStatus::Unavailable
    );
}

#[test]
fn bloat_proxy_from_dead_tuple_ratio() {
    let mut bundle = empty_bundle();
    bundle.table_stats = vec![TableStat {
        schema: "public".to_string(),
        table: "big".to_string(),
        seq_scan: 0,
        idx_scan: 0,
        n_live_tup: 3000,
        n_dead_tup: 7000,
        last_vacuum: None,
        last_autovacuum: None,
        total_size_bytes: 0,
    }];
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::BloatProxies),
        DbHealthStatus::Critical
    );
}

#[test]
fn small_tables_excluded_from_bloat_proxy() {
    let mut bundle = empty_bundle();
    bundle.table_stats = vec![TableStat {
        schema: "public".to_string(),
        table: "tiny".to_string(),
        seq_scan: 0,
        idx_scan: 0,
        n_live_tup: 5,
        n_dead_tup: 900,
        last_vacuum: None,
        last_autovacuum: None,
        total_size_bytes: 0,
    }];
    let verdict = classify_db(&bundle, now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::BloatProxies),
        DbHealthStatus::Unavailable
    );
}

#[test]
fn unavailable_verdict_marks_availability_critical_and_everything_else_unavailable() {
    let verdict = unavailable_verdict("connection refused", now());
    assert_eq!(
        status_of(&verdict, DbHealthCategory::Availability),
        DbHealthStatus::Critical
    );
    assert_eq!(
        status_of(&verdict, DbHealthCategory::ConnectionSaturation),
        DbHealthStatus::Unavailable
    );
    assert_eq!(verdict.score, 0);
}
