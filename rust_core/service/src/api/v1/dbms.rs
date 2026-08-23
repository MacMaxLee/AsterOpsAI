//! Unit U31's first direct wire surface for `core::dbms::DbmsAdapter`
//! (fully built since unit U20, but until now consumed only internally
//! by `analysis.rs`'s own correlation endpoint, folded into a ranked
//! verdict — never exposed raw). `list_sessions`/`lock_graph` are the
//! two most fundamental "what's happening right now" views, both
//! already proven at the `core` level (correlation's own lock-storm
//! detector already exercises `lock_graph` for real). See docs/adr/0036
//! for why this is a deliberately small first slice, not all 12
//! `DbmsAdapter` methods.

use std::sync::Arc;

use ai_ops_core::dbms::{
    log_tail, DbmsAdapter, DeadlockInfo as CoreDeadlockInfo, Gated, GucValue as CoreGucValue,
    IdleInTransactionSession as CoreIdleInTransactionSession, IndexStat as CoreIndexStat,
    LockEdge as CoreLockEdge, LongTransaction as CoreLongTransaction, QueryStat as CoreQueryStat,
    ReplicationStatus as CoreReplicationStatus, SessionInfo as CoreSessionInfo, SessionState,
    StandbyInfo as CoreStandbyInfo, TableStat as CoreTableStat,
    TempFileActivity as CoreTempFileActivity,
};
use ai_ops_core::repository;
use ai_ops_core::security;
use axum::extract::{Extension, State};
use chrono::Utc;
use contracts::{
    ApiError, DeadlockInfo, GatedValue, GucValue, IdleInTransactionSession, IndexStat, LockEdge,
    LongTransaction, QueryStat, ReplicationStatus, SessionInfo, StandbyInfo, TableStat,
    TempFileActivity,
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

/// Unit U57 (SRS FR-DBSEC-001(d)): unlike U55/U56's own necessarily
/// pragmatic choice of home (`gucs`, since no query for GUC/role data
/// existed to reuse and no endpoint was a perfect semantic fit), this
/// is the one perfect fit — `sessions` already surfaces `client_addr`
/// directly, no new query needed. Same best-effort posture: a
/// detection failure is logged, never blocks the real session data
/// this endpoint exists to serve. `detect_unusual_client_address`
/// fires on a genuinely new address's very first observation (not a
/// diff — see its own doc comment) — a real, accepted tradeoff worth
/// naming: a fresh deployment's first poll flags every already-
/// connected address as "new" to this service's own tracking, the
/// same characteristic `detect_untrusted_device` has always had for
/// already-attached devices.
async fn check_unusual_client_addresses(
    repo: &repository::RepositoryHandle,
    sessions: &[CoreSessionInfo],
) {
    let now = Utc::now();
    for session in sessions {
        let Some(client_addr) = session.client_addr.as_deref() else {
            continue;
        };
        let already_known =
            match repository::record_client_address_seen(repo, client_addr, now).await {
                Ok(already_known) => already_known,
                Err(err) => {
                    tracing::warn!(error = %err, client_addr, "record_client_address_seen failed");
                    continue;
                }
            };
        let Some(event) = security::detect_unusual_client_address(client_addr, already_known, now)
        else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, client_addr, "record_event failed for a detected unusual client address");
        }
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

        if let Some(repo) = state.repository.as_ref() {
            check_unusual_client_addresses(repo, &sessions).await;
        }

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

/// Unit U55 (SRS FR-DBSEC-001(e)): the first production trigger for
/// `core::security`'s detection→incident pipeline (neither pre-existing
/// detector was ever called from `service` before this unit). No
/// standing DBMS poll loop exists — detection piggybacks on this
/// already-real, already-polled endpoint instead of new scheduler
/// infrastructure. Best-effort: a detection failure is logged, never
/// blocks the real GUC data this endpoint exists to serve.
async fn check_guc_changes(repo: &repository::RepositoryHandle, gucs: &[CoreGucValue]) {
    let now = Utc::now();
    for guc in gucs {
        let previous = match repository::record_guc_value(repo, &guc.name, &guc.setting, now).await
        {
            Ok(previous) => previous,
            Err(err) => {
                tracing::warn!(error = %err, guc = %guc.name, "record_guc_value failed");
                continue;
            }
        };
        let Some(event) =
            security::detect_guc_change(&guc.name, previous.as_deref(), &guc.setting, now)
        else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, guc = %guc.name, "record_event failed for a detected GUC change");
        }
    }
}

/// Unit U56 (SRS FR-DBSEC-001(b), narrowed to the `rolsuper` flag —
/// see `security::detect_role_superuser_granted`'s own doc comment).
/// No existing endpoint is a perfect semantic fit for role data;
/// `gucs` — already U55's "slow-changing DB-wide security/config
/// posture" poll point — is the least-contrived existing, already-
/// polled home, a pragmatic choice named explicitly rather than a
/// perfect one. Same best-effort posture as `check_guc_changes`.
async fn check_role_superuser_grants(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
) {
    let roles = match adapter.role_superuser_flags().await {
        Ok(roles) => roles,
        Err(err) => {
            tracing::warn!(error = %err, "role_superuser_flags failed");
            return;
        }
    };
    let now = Utc::now();
    for role in roles {
        let previous = match repository::record_role_superuser_flag(
            repo,
            &role.rolname,
            role.rolsuper,
            now,
        )
        .await
        {
            Ok(previous) => previous,
            Err(err) => {
                tracing::warn!(error = %err, rolname = %role.rolname, "record_role_superuser_flag failed");
                continue;
            }
        };
        let Some(event) =
            security::detect_role_superuser_granted(&role.rolname, previous, role.rolsuper, now)
        else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, rolname = %role.rolname, "record_event failed for a detected superuser grant");
        }
    }
}

/// Unit U58 (SRS FR-DBSEC-001(b)'s deferred remainder — see `security::
/// detect_role_membership_granted`'s own doc comment): the third
/// best-effort step on this same "DB-wide security/config posture"
/// poll point.
async fn check_role_membership_grants(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
) {
    let memberships = match adapter.role_memberships().await {
        Ok(memberships) => memberships,
        Err(err) => {
            tracing::warn!(error = %err, "role_memberships failed");
            return;
        }
    };
    let now = Utc::now();
    for membership in memberships {
        let already_known = match repository::record_role_membership_seen(
            repo,
            &membership.member,
            &membership.granted_role,
            now,
        )
        .await
        {
            Ok(already_known) => already_known,
            Err(err) => {
                tracing::warn!(error = %err, member = %membership.member, granted_role = %membership.granted_role, "record_role_membership_seen failed");
                continue;
            }
        };
        let Some(event) = security::detect_role_membership_granted(
            &membership.member,
            &membership.granted_role,
            already_known,
            now,
        ) else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, member = %membership.member, "record_event failed for a detected role-membership grant");
        }
    }
}

/// Unit U59 (SRS FR-DBSEC-001(c), deliberately narrowed — see
/// `security::detect_table_privilege_granted`'s own doc comment): the
/// fourth best-effort step on this same "DB-wide security/config
/// posture" poll point.
async fn check_table_privilege_grants(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
) {
    let grants = match adapter.table_privilege_grants().await {
        Ok(grants) => grants,
        Err(err) => {
            tracing::warn!(error = %err, "table_privilege_grants failed");
            return;
        }
    };
    let now = Utc::now();
    for grant in grants {
        let already_known = match repository::record_table_privilege_grant_seen(
            repo,
            &grant.grantee,
            &grant.schema,
            &grant.table,
            &grant.privilege_type,
            now,
        )
        .await
        {
            Ok(already_known) => already_known,
            Err(err) => {
                tracing::warn!(error = %err, grantee = %grant.grantee, table = %grant.table, "record_table_privilege_grant_seen failed");
                continue;
            }
        };
        let Some(event) = security::detect_table_privilege_granted(
            &grant.grantee,
            &grant.schema,
            &grant.table,
            &grant.privilege_type,
            already_known,
            now,
        ) else {
            continue;
        };
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, grantee = %grant.grantee, "record_event failed for a detected table privilege grant");
        }
    }
}

/// Unit U60 (SRS FR-DBSEC-001(a), the last FR-DBSEC-001 sub-item —
/// see `security::detect_auth_failure`'s own doc comment and
/// docs/adr/0065): the fifth best-effort step on this same "DB-wide
/// security/config posture" poll point (`gucs`'s handler has now
/// accumulated five different concerns, a real, named design debt
/// worth a future dedicated refactor — not attempted in this unit).
/// Unlike the other four checks, this does real local-host file I/O
/// (`core::dbms::log_tail`, deliberately not a `DbmsAdapter` method —
/// see that module's own doc comment) and simply never fires
/// anything on a non-co-located deployment (`csv_logging_enabled`
/// false, or the log directory unreadable/empty) — every other check
/// keeps working regardless.
async fn check_auth_failures(repo: &repository::RepositoryHandle, adapter: &Arc<dyn DbmsAdapter>) {
    let config = match adapter.auth_failure_log_config().await {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(error = %err, "auth_failure_log_config failed");
            return;
        }
    };
    if !config.csv_logging_enabled {
        return;
    }

    let active_file = match log_tail::find_active_csv_file(&config.log_dir).await {
        Ok(Some(path)) => path,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(error = %err, log_dir = %config.log_dir.display(), "find_active_csv_file failed");
            return;
        }
    };
    let active_file_str = active_file.to_string_lossy().to_string();

    let previous_offset = match repository::get_log_tail_offset(repo, &active_file_str).await {
        Ok(offset) => offset,
        Err(err) => {
            tracing::warn!(error = %err, log_file = %active_file_str, "get_log_tail_offset failed");
            return;
        }
    };
    let since_offset = previous_offset.map(|offset| (active_file.clone(), offset));

    let (events, tailed_file, new_offset) = match log_tail::read_new_auth_failures(
        &config.log_dir,
        since_offset,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::warn!(error = %err, log_dir = %config.log_dir.display(), "read_new_auth_failures failed");
            return;
        }
    };

    let now = Utc::now();
    for auth_failure in &events {
        let event = security::detect_auth_failure(auth_failure, now);
        if let Err(err) = security::record_event(repo, event).await {
            tracing::warn!(error = %err, "record_event failed for a detected auth failure");
        }
    }

    let tailed_file_str = tailed_file.to_string_lossy().to_string();
    if let Err(err) = repository::set_log_tail_offset(repo, tailed_file_str, new_offset, now).await
    {
        tracing::warn!(error = %err, log_file = %active_file_str, "set_log_tail_offset failed");
    }
}

/// Unit U71 (ADR 0065's own named follow-up): the five best-effort
/// checks `gucs` piggybacked one unit at a time (U55-U60), extracted
/// into their own named function — same checks, same order, same
/// inputs, no behavior change. `gucs` still owns the doc comment
/// explaining *why* this lives here (no dedicated poll/scheduler
/// infrastructure exists in `service` yet), just no longer inlines the
/// five-call body itself.
async fn run_security_config_sweep(
    repo: &repository::RepositoryHandle,
    adapter: &Arc<dyn DbmsAdapter>,
    gucs: &[CoreGucValue],
) {
    check_guc_changes(repo, gucs).await;
    check_role_superuser_grants(repo, adapter).await;
    check_role_membership_grants(repo, adapter).await;
    check_table_privilege_grants(repo, adapter).await;
    check_auth_failures(repo, adapter).await;
}

/// This handler has grown, one unit at a time (U55-U60), into a de
/// facto "DB-wide security/config sweep" — it now does meaningfully
/// more than its name ("read the relevant GUCs") suggests, since no
/// dedicated poll/scheduler infrastructure exists in `service` yet for
/// `run_security_config_sweep`'s five best-effort checks to live on
/// instead.
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

        if let Some(repo) = state.repository.as_ref() {
            run_security_config_sweep(repo, &adapter, &gucs).await;
        }

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

fn to_wire_long_transaction(txn: CoreLongTransaction) -> LongTransaction {
    LongTransaction {
        pid: txn.pid,
        username: txn.username,
        duration_seconds: txn.duration_seconds,
        state: to_wire_state(txn.state),
        query: txn.query,
    }
}

pub async fn long_transactions(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<LongTransaction>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let txns = adapter.long_transactions().await.map_err(|err| {
            tracing::warn!(error = %err, "long_transactions failed");
            ApiError::Unavailable(format!("DB poll failed: {err}"))
        })?;
        Ok(txns.into_iter().map(to_wire_long_transaction).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

fn to_wire_idle_in_transaction_session(
    session: CoreIdleInTransactionSession,
) -> IdleInTransactionSession {
    IdleInTransactionSession {
        pid: session.pid,
        username: session.username,
        idle_duration_seconds: session.idle_duration_seconds,
    }
}

pub async fn idle_in_transaction_sessions(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<IdleInTransactionSession>> {
    let result = async {
        let adapter = state.dbms_adapter.clone().ok_or_else(|| {
            ApiError::Unavailable("no database connection configured".to_string())
        })?;
        let sessions = adapter
            .idle_in_transaction_sessions()
            .await
            .map_err(|err| {
                tracing::warn!(error = %err, "idle_in_transaction_sessions failed");
                ApiError::Unavailable(format!("DB poll failed: {err}"))
            })?;
        Ok(sessions
            .into_iter()
            .map(to_wire_idle_in_transaction_session)
            .collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}
