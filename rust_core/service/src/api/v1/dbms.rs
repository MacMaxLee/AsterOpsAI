//! Unit U31's first direct wire surface for `core::dbms::DbmsAdapter`
//! (fully built since unit U20, but until now consumed only internally
//! by `analysis.rs`'s own correlation endpoint, folded into a ranked
//! verdict — never exposed raw). `list_sessions`/`lock_graph` are the
//! two most fundamental "what's happening right now" views, both
//! already proven at the `core` level (correlation's own lock-storm
//! detector already exercises `lock_graph` for real). See docs/adr/0036
//! for why this is a deliberately small first slice, not all 12
//! `DbmsAdapter` methods.

use ai_ops_core::dbms::{
    DeadlockInfo as CoreDeadlockInfo, Gated, GucValue as CoreGucValue, IndexStat as CoreIndexStat,
    LockEdge as CoreLockEdge, QueryStat as CoreQueryStat,
    ReplicationStatus as CoreReplicationStatus, SessionInfo as CoreSessionInfo, SessionState,
    StandbyInfo as CoreStandbyInfo, TableStat as CoreTableStat,
    TempFileActivity as CoreTempFileActivity,
};
use axum::extract::{Extension, State};
use contracts::{
    ApiError, DeadlockInfo, GatedValue, GucValue, IndexStat, LockEdge, QueryStat,
    ReplicationStatus, SessionInfo, StandbyInfo, TableStat, TempFileActivity,
};

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

fn to_wire_state(state: SessionState) -> contracts::SessionState {
    match state {
        SessionState::Active => contracts::SessionState::Active,
        SessionState::Idle => contracts::SessionState::Idle,
        SessionState::IdleInTransaction => contracts::SessionState::IdleInTransaction,
        SessionState::Waiting => contracts::SessionState::Waiting,
    }
}

fn to_wire_session(session: CoreSessionInfo) -> SessionInfo {
    SessionInfo {
        pid: session.pid,
        username: session.username,
        database: session.database,
        state: to_wire_state(session.state),
        client_addr: session.client_addr,
        xact_start: session.xact_start,
        query_start: session.query_start,
        query: session.query,
    }
}

fn to_wire_lock(lock: CoreLockEdge) -> LockEdge {
    LockEdge {
        blocked_pid: lock.blocked_pid,
        blocked_query: lock.blocked_query,
        blocking_pid: lock.blocking_pid,
        blocking_query: lock.blocking_query,
        lock_type: lock.lock_type,
    }
}

/// No DB configured, or a genuine live poll failure, both degrade to a
/// real, honest `Unavailable` — the same category `analysis.rs`'s own
/// `compute_db_verdict` already uses for "no DB evidence" (its own
/// exact wording for the unconfigured case), not a `500` for a
/// condition this endpoint has no business treating as a server bug.
pub async fn sessions(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<SessionInfo>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let sessions = adapter.list_sessions().await.map_err(|err| {
            tracing::warn!(error = %err, "list_sessions failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(sessions.into_iter().map(to_wire_session).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_query_stat(stat: CoreQueryStat) -> QueryStat {
    QueryStat {
        query_fingerprint: stat.query_fingerprint,
        normalized_query: stat.normalized_query,
        calls: stat.calls,
        total_exec_time_ms: stat.total_exec_time_ms,
        mean_exec_time_ms: stat.mean_exec_time_ms,
        rows: stat.rows,
    }
}

fn to_wire_gated_query_stats(gated: Gated<Vec<CoreQueryStat>>) -> GatedValue<Vec<QueryStat>> {
    match gated {
        Gated::Supported(stats) => GatedValue::Supported {
            value: stats.into_iter().map(to_wire_query_stat).collect(),
        },
        Gated::Limited { reason } => GatedValue::Limited { reason },
        Gated::Unavailable { reason } => GatedValue::Unavailable { reason },
        Gated::PermissionRequired { reason } => GatedValue::PermissionRequired { reason },
    }
}

/// Unlike `sessions`/`locks`, a successful poll's own real `Gated<T>`
/// result (e.g. `pg_stat_statements` genuinely not installed) is
/// returned as real `200 OK` data, not folded into `Unavailable` —
/// only "the DBMS feature is unreachable at all" (no DB configured, or
/// the poll itself failed) stays a `503`. See docs/adr/0038.
pub async fn query_stats(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<GatedValue<Vec<QueryStat>>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let gated = adapter.query_stats().await.map_err(|err| {
            tracing::warn!(error = %err, "query_stats failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(to_wire_gated_query_stats(gated))
    }
    .await;

    ApiResponse::new(request_id, result)
}

pub async fn locks(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<LockEdge>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let locks = adapter.lock_graph().await.map_err(|err| {
            tracing::warn!(error = %err, "lock_graph failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(locks.into_iter().map(to_wire_lock).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_table_stat(stat: CoreTableStat) -> TableStat {
    TableStat {
        schema: stat.schema,
        table: stat.table,
        seq_scan: stat.seq_scan,
        idx_scan: stat.idx_scan,
        n_live_tup: stat.n_live_tup,
        n_dead_tup: stat.n_dead_tup,
        last_vacuum: stat.last_vacuum,
        last_autovacuum: stat.last_autovacuum,
        total_size_bytes: stat.total_size_bytes,
    }
}

pub async fn table_stats(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<TableStat>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let stats = adapter.table_stats().await.map_err(|err| {
            tracing::warn!(error = %err, "table_stats failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(stats.into_iter().map(to_wire_table_stat).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_index_stat(stat: CoreIndexStat) -> IndexStat {
    IndexStat {
        schema: stat.schema,
        table: stat.table,
        index: stat.index,
        idx_scan: stat.idx_scan,
        size_bytes: stat.size_bytes,
    }
}

pub async fn index_stats(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<IndexStat>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let stats = adapter.index_stats().await.map_err(|err| {
            tracing::warn!(error = %err, "index_stats failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(stats.into_iter().map(to_wire_index_stat).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_standby(standby: CoreStandbyInfo) -> StandbyInfo {
    StandbyInfo {
        client_addr: standby.client_addr,
        state: standby.state,
        sent_lsn: standby.sent_lsn,
        write_lsn: standby.write_lsn,
        flush_lsn: standby.flush_lsn,
        replay_lsn: standby.replay_lsn,
        replay_lag_seconds: standby.replay_lag_seconds,
    }
}

fn to_wire_replication_status(status: CoreReplicationStatus) -> ReplicationStatus {
    ReplicationStatus {
        is_primary: status.is_primary,
        in_recovery: status.in_recovery,
        standbys: status.standbys.into_iter().map(to_wire_standby).collect(),
    }
}

/// Unlike every other `/dbms/*` endpoint so far, a successful poll's
/// payload is a single object, not a `Vec<T>` — the 503-vs-200 split
/// itself is otherwise identical. See docs/adr/0042.
pub async fn replication(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<ReplicationStatus> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let status = adapter.replication_status().await.map_err(|err| {
            tracing::warn!(error = %err, "replication_status failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(to_wire_replication_status(status))
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_guc(guc: CoreGucValue) -> GucValue {
    GucValue {
        name: guc.name,
        setting: guc.setting,
        unit: guc.unit,
        source: guc.source,
    }
}

pub async fn gucs(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<GucValue>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let gucs = adapter.relevant_gucs().await.map_err(|err| {
            tracing::warn!(error = %err, "relevant_gucs failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(gucs.into_iter().map(to_wire_guc).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_temp_file_activity(activity: CoreTempFileActivity) -> TempFileActivity {
    TempFileActivity {
        temp_files: activity.temp_files,
        temp_bytes: activity.temp_bytes,
        stats_reset: activity.stats_reset,
    }
}

pub async fn temp_file_activity(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<TempFileActivity> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let activity = adapter.temp_file_activity().await.map_err(|err| {
            tracing::warn!(error = %err, "temp_file_activity failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(to_wire_temp_file_activity(activity))
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_deadlock_info(info: CoreDeadlockInfo) -> DeadlockInfo {
    DeadlockInfo {
        deadlocks: info.deadlocks,
        stats_reset: info.stats_reset,
    }
}

pub async fn deadlock_history(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<DeadlockInfo> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let info = adapter.deadlock_history().await.map_err(|err| {
            tracing::warn!(error = %err, "deadlock_history failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(to_wire_deadlock_info(info))
    }
    .await;

    ApiResponse::new(request_id, result)
}
