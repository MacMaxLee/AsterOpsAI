//! All SQL string literals for the PostgreSQL adapter live here — the
//! `check-no-sql-outside-adapters.sh` grep gate enforces that nothing
//! outside `core/src/dbms/adapters/` contains one.

use crate::dbms::{
    capability::Gated, privacy, DatabaseInfo, DbmsError, DeadlockInfo, GucValue,
    IdleInTransactionSession, IndexStat, LockEdge, LongTransaction, QueryStat, ReplicationStatus,
    SessionInfo, SessionState, StandbyInfo, TableStat, TempFileActivity, VersionInfo,
};

pub async fn version_info(client: &tokio_postgres::Client) -> Result<VersionInfo, DbmsError> {
    let row = client
        .query_one(
            "SELECT version(), current_setting('server_version_num')::int, \
             pg_postmaster_start_time()",
            &[],
        )
        .await?;
    Ok(VersionInfo {
        version_string: row.get(0),
        server_version_num: row.get(1),
        started_at: row.get(2),
    })
}

pub async fn list_databases(
    client: &tokio_postgres::Client,
) -> Result<Vec<DatabaseInfo>, DbmsError> {
    let rows = client
        .query(
            "SELECT datname, pg_database_size(datname) FROM pg_database \
             WHERE datistemplate = false ORDER BY datname",
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| DatabaseInfo {
            name: row.get(0),
            size_bytes: row.get(1),
        })
        .collect())
}

fn classify_session_state(state: Option<&str>, wait_event_type: Option<&str>) -> SessionState {
    if wait_event_type.is_some() {
        return SessionState::Waiting;
    }
    match state {
        Some("active") => SessionState::Active,
        // Covers both "idle in transaction" and "idle in transaction
        // (aborted)" — both are the same operational concern (a session
        // holding a transaction open).
        Some(s) if s.starts_with("idle in transaction") => SessionState::IdleInTransaction,
        _ => SessionState::Idle,
    }
}

pub async fn list_sessions(
    client: &tokio_postgres::Client,
    capture_raw_sql: bool,
) -> Result<Vec<SessionInfo>, DbmsError> {
    let rows = client
        .query(
            "SELECT pid, usename, datname, state, wait_event_type, client_addr::text, \
             xact_start, query_start, query \
             FROM pg_stat_activity WHERE pid != pg_backend_pid()",
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let state: Option<String> = row.get(3);
            let wait_event_type: Option<String> = row.get(4);
            let raw_query: Option<String> = row.get(8);
            SessionInfo {
                pid: row.get(0),
                username: row.get(1),
                database: row.get(2),
                state: classify_session_state(state.as_deref(), wait_event_type.as_deref()),
                client_addr: row.get(5),
                xact_start: row.get(6),
                query_start: row.get(7),
                query: privacy::sanitize_query(raw_query.as_deref(), capture_raw_sql),
            }
        })
        .collect())
}

pub async fn pg_stat_statements_available(
    client: &tokio_postgres::Client,
) -> Result<bool, DbmsError> {
    let row = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements')",
            &[],
        )
        .await?;
    Ok(row.get(0))
}

pub async fn query_stats(
    client: &tokio_postgres::Client,
) -> Result<Gated<Vec<QueryStat>>, DbmsError> {
    if !pg_stat_statements_available(client).await? {
        return Ok(Gated::Unavailable {
            reason: "pg_stat_statements extension is not installed on this instance".to_string(),
        });
    }
    let rows = client
        .query(
            "SELECT queryid::text, query, calls, total_exec_time, mean_exec_time, rows \
             FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 200",
            &[],
        )
        .await?;
    Ok(Gated::Supported(
        rows.into_iter()
            .map(|row| QueryStat {
                query_fingerprint: row.get(0),
                normalized_query: row.get(1),
                calls: row.get(2),
                total_exec_time_ms: row.get(3),
                mean_exec_time_ms: row.get(4),
                rows: row.get(5),
            })
            .collect(),
    ))
}

pub async fn lock_graph(client: &tokio_postgres::Client) -> Result<Vec<LockEdge>, DbmsError> {
    // Every blocked backend, paired with each PID actually blocking it
    // (`pg_blocking_pids`, not just "someone holds a conflicting lock" —
    // that function already resolves the real blocking chain, including
    // transitive waits-for-a-waiter cases). SRS FR-DB-006 explicitly wants
    // the blocking side identified, not merely the blocked one.
    let rows = client
        .query(
            "SELECT blocked.pid, blocked.query, blocking.pid, blocking.query, \
             blocked_locks.mode \
             FROM pg_stat_activity blocked \
             JOIN pg_locks blocked_locks ON blocked_locks.pid = blocked.pid \
             AND NOT blocked_locks.granted \
             JOIN LATERAL unnest(pg_blocking_pids(blocked.pid)) AS blocking_pid ON true \
             JOIN pg_stat_activity blocking ON blocking.pid = blocking_pid",
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let blocked_query: Option<String> = row.get(1);
            let blocking_query: Option<String> = row.get(3);
            LockEdge {
                blocked_pid: row.get(0),
                blocked_query: privacy::sanitize_query(blocked_query.as_deref(), false),
                blocking_pid: row.get(2),
                blocking_query: privacy::sanitize_query(blocking_query.as_deref(), false),
                lock_type: row.get(4),
            }
        })
        .collect())
}

pub async fn table_stats(client: &tokio_postgres::Client) -> Result<Vec<TableStat>, DbmsError> {
    let rows = client
        .query(
            "SELECT schemaname, relname, seq_scan, COALESCE(idx_scan, 0), n_live_tup, \
             n_dead_tup, last_vacuum, last_autovacuum, \
             pg_total_relation_size(schemaname || '.' || relname) \
             FROM pg_stat_user_tables ORDER BY schemaname, relname",
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| TableStat {
            schema: row.get(0),
            table: row.get(1),
            seq_scan: row.get(2),
            idx_scan: row.get(3),
            n_live_tup: row.get(4),
            n_dead_tup: row.get(5),
            last_vacuum: row.get(6),
            last_autovacuum: row.get(7),
            total_size_bytes: row.get(8),
        })
        .collect())
}

pub async fn index_stats(client: &tokio_postgres::Client) -> Result<Vec<IndexStat>, DbmsError> {
    let rows = client
        .query(
            "SELECT schemaname, relname, indexrelname, idx_scan, \
             pg_relation_size(indexrelid) \
             FROM pg_stat_user_indexes ORDER BY schemaname, relname, indexrelname",
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| IndexStat {
            schema: row.get(0),
            table: row.get(1),
            index: row.get(2),
            idx_scan: row.get(3),
            size_bytes: row.get(4),
        })
        .collect())
}

pub async fn replication_status(
    client: &tokio_postgres::Client,
) -> Result<ReplicationStatus, DbmsError> {
    let in_recovery: bool = client
        .query_one("SELECT pg_is_in_recovery()", &[])
        .await?
        .get(0);

    if in_recovery {
        // This instance is itself a standby — it has no pg_stat_replication
        // rows of its own to report (those live on the primary).
        return Ok(ReplicationStatus {
            is_primary: false,
            in_recovery: true,
            standbys: Vec::new(),
        });
    }

    let rows = client
        .query(
            "SELECT client_addr::text, state, sent_lsn::text, write_lsn::text, \
             flush_lsn::text, replay_lsn::text, \
             EXTRACT(EPOCH FROM (now() - replay_lag))::float8 \
             FROM pg_stat_replication",
            &[],
        )
        .await?;
    let standbys = rows
        .into_iter()
        .map(|row| StandbyInfo {
            client_addr: row.get(0),
            state: row.get(1),
            sent_lsn: row.get(2),
            write_lsn: row.get(3),
            flush_lsn: row.get(4),
            replay_lsn: row.get(5),
            replay_lag_seconds: row.get(6),
        })
        .collect();
    Ok(ReplicationStatus {
        is_primary: true,
        in_recovery: false,
        standbys,
    })
}

/// Curated, not exhaustive (requirement 5's "relevant GUCs", not the full
/// `pg_settings` surface).
const RELEVANT_GUCS: &[&str] = &[
    "max_connections",
    "shared_buffers",
    "work_mem",
    "effective_cache_size",
    "wal_level",
    "max_wal_size",
    "checkpoint_timeout",
    "autovacuum",
    "statement_timeout",
];

pub async fn relevant_gucs(client: &tokio_postgres::Client) -> Result<Vec<GucValue>, DbmsError> {
    let rows = client
        .query(
            "SELECT name, setting, unit, source FROM pg_settings WHERE name = ANY($1)",
            &[&RELEVANT_GUCS],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| GucValue {
            name: row.get(0),
            setting: row.get(1),
            unit: row.get(2),
            source: row.get(3),
        })
        .collect())
}

pub async fn temp_file_activity(
    client: &tokio_postgres::Client,
) -> Result<TempFileActivity, DbmsError> {
    let row = client
        .query_one(
            "SELECT temp_files, temp_bytes, stats_reset FROM pg_stat_database \
             WHERE datname = current_database()",
            &[],
        )
        .await?;
    Ok(TempFileActivity {
        temp_files: row.get(0),
        temp_bytes: row.get(1),
        stats_reset: row.get(2),
    })
}

pub async fn deadlock_history(client: &tokio_postgres::Client) -> Result<DeadlockInfo, DbmsError> {
    let row = client
        .query_one(
            "SELECT deadlocks, stats_reset FROM pg_stat_database \
             WHERE datname = current_database()",
            &[],
        )
        .await?;
    Ok(DeadlockInfo {
        deadlocks: row.get(0),
        stats_reset: row.get(1),
    })
}

pub async fn long_transactions(
    client: &tokio_postgres::Client,
    threshold_seconds: f64,
    capture_raw_sql: bool,
) -> Result<Vec<LongTransaction>, DbmsError> {
    let rows = client
        .query(
            "SELECT pid, usename, EXTRACT(EPOCH FROM (now() - xact_start))::float8, \
             state, wait_event_type, query \
             FROM pg_stat_activity \
             WHERE xact_start IS NOT NULL \
             AND state != 'idle' \
             AND EXTRACT(EPOCH FROM (now() - xact_start)) >= $1::double precision \
             AND pid != pg_backend_pid()",
            &[&threshold_seconds],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let state: Option<String> = row.get(3);
            let wait_event_type: Option<String> = row.get(4);
            let raw_query: Option<String> = row.get(5);
            LongTransaction {
                pid: row.get(0),
                username: row.get(1),
                duration_seconds: row.get(2),
                state: classify_session_state(state.as_deref(), wait_event_type.as_deref()),
                query: privacy::sanitize_query(raw_query.as_deref(), capture_raw_sql),
            }
        })
        .collect())
}

pub async fn idle_in_transaction_sessions(
    client: &tokio_postgres::Client,
    threshold_seconds: f64,
) -> Result<Vec<IdleInTransactionSession>, DbmsError> {
    let rows = client
        .query(
            "SELECT pid, usename, EXTRACT(EPOCH FROM (now() - state_change))::float8 \
             FROM pg_stat_activity \
             WHERE state LIKE 'idle in transaction%' \
             AND EXTRACT(EPOCH FROM (now() - state_change)) >= $1::double precision \
             AND pid != pg_backend_pid()",
            &[&threshold_seconds],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| IdleInTransactionSession {
            pid: row.get(0),
            username: row.get(1),
            idle_duration_seconds: row.get(2),
        })
        .collect())
}

/// Used by `role_check`'s validation query and by tests — kept here since
/// it's still a SQL string literal, even though it isn't one of the 13
/// trait methods.
pub async fn current_user_is_superuser(client: &tokio_postgres::Client) -> Result<bool, DbmsError> {
    let row = client
        .query_one(
            "SELECT rolsuper FROM pg_roles WHERE rolname = current_user",
            &[],
        )
        .await?;
    Ok(row.get(0))
}
