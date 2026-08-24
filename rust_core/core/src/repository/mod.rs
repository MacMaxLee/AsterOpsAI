//! SQLite persistence: bounded-growth telemetry history and a
//! tamper-evident audit log (unit U2). See docs/TRS.md §12-14 and
//! docs/adr/0007-sqlite-single-writer-task-no-writer-pool.md.

pub mod audit;
pub mod benchmark;
pub mod client_address_history;
pub mod connection;
pub mod device_trust;
pub mod error;
pub mod guc_history;
pub mod history;
pub mod log_tail_offset;
pub mod migrations;
pub mod models;
pub mod performance_analysis;
pub mod policy;
pub mod reader;
pub mod retention;
pub mod role_history;
pub mod role_membership_history;
pub mod security;
pub mod table_privilege_history;
pub mod telemetry_store;
mod time;
pub mod tuning;
pub mod writer;

use std::path::PathBuf;

pub use connection::ReadPool;
pub use error::RepositoryError;
pub use history::{
    query_cpu_history, query_memory_history, query_network_history, query_recent_snapshots,
    query_storage_history, HistoryRange, ResolvedRange,
};
pub use models::{
    AuditEventRecorded, BenchmarkRunRow, ChainVerification, NewAuditEvent, NewBenchmarkRun,
    NewProposedAction, NewSecurityEvent, NewSecuritySuppression, NewTuningPlan,
    PerformanceAnalysisRow, PolicyActionRow, RetentionAuditDetail, RetentionReport,
    SecurityEventRow, SecurityIncidentRow, TelemetrySnapshotRow, TransitionPatch, TuningPlanRow,
};
pub use writer::WriteCommand;

use writer::CommandSender;

#[derive(Debug, Clone)]
pub struct RepositoryConfig {
    pub db_path: PathBuf,
}

#[derive(Clone)]
pub struct RepositoryHandle {
    pub command_tx: CommandSender,
    pub read_pool: ReadPool,
}

/// Opens (creating if needed) the database at `config.db_path`, applies
/// pragmas, runs migrations, and spawns the writer thread. A migration
/// failure returns `Err` without ever dropping or recreating the file
/// (requirement 3) — callers should degrade to running without persistence,
/// never crash the process over this.
pub fn init(config: &RepositoryConfig) -> Result<RepositoryHandle, RepositoryError> {
    let mut write_conn = connection::open_write_connection(&config.db_path)?;
    migrations::run(&mut write_conn)?;

    let read_pool = connection::open_read_pool(&config.db_path)?;
    let (command_tx, _writer_thread) = writer::spawn(write_conn)?;

    Ok(RepositoryHandle {
        command_tx,
        read_pool,
    })
}

/// Fire-and-forget: never blocks, never awaited. A full channel or a
/// writer that isn't running just means the sample is dropped (logged at
/// `debug!`) — persistence must never stall the sampler tick.
pub fn try_persist_telemetry_snapshot(handle: &RepositoryHandle, row: TelemetrySnapshotRow) {
    if handle
        .command_tx
        .try_send(WriteCommand::InsertTelemetrySnapshot(Box::new(row)))
        .is_err()
    {
        tracing::debug!("telemetry snapshot dropped: writer channel full or unavailable");
    }
}

/// Fire-and-forget, same reasoning as `try_persist_telemetry_snapshot`:
/// performance-analysis results are periodic derived data, not something a
/// caller needs to await the durability of.
pub fn try_persist_performance_analysis(handle: &RepositoryHandle, row: PerformanceAnalysisRow) {
    if handle
        .command_tx
        .try_send(WriteCommand::InsertPerformanceAnalysis(Box::new(row)))
        .is_err()
    {
        tracing::debug!("performance analysis result dropped: writer channel full or unavailable");
    }
}

pub async fn record_audit_event(
    handle: &RepositoryHandle,
    new: NewAuditEvent,
) -> Result<AuditEventRecorded, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::InsertAuditEvent {
            new,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn run_retention_sweep(
    handle: &RepositoryHandle,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<RetentionReport, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RunRetentionSweep {
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn propose_action(
    handle: &RepositoryHandle,
    new: NewProposedAction,
) -> Result<PolicyActionRow, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::PolicyPropose {
            new,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// See `repository::policy::transition`'s own doc comment for the
/// compare-and-swap semantics this wraps.
#[allow(clippy::too_many_arguments)]
pub async fn transition_action(
    handle: &RepositoryHandle,
    id: i64,
    expected_status: impl Into<String>,
    new_status: impl Into<String>,
    now: chrono::DateTime<chrono::Utc>,
    check_not_expired: bool,
    patch: TransitionPatch,
) -> Result<PolicyActionRow, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::PolicyTransition {
            id,
            expected_status: expected_status.into(),
            new_status: new_status.into(),
            now,
            check_not_expired,
            patch,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn get_action(
    handle: &RepositoryHandle,
    id: i64,
) -> Result<Option<PolicyActionRow>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    policy::get_by_id(&conn, id)
}

pub async fn insert_benchmark_run(
    handle: &RepositoryHandle,
    new: NewBenchmarkRun,
) -> Result<BenchmarkRunRow, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::BenchmarkRunInsert {
            new,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn mark_benchmark_run_rolled_back(
    handle: &RepositoryHandle,
    id: i64,
    rollback_action_id: i64,
) -> Result<BenchmarkRunRow, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::BenchmarkRunMarkRolledBack {
            id,
            rollback_action_id,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn get_benchmark_run(
    handle: &RepositoryHandle,
    id: i64,
) -> Result<Option<BenchmarkRunRow>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    benchmark::get_by_id(&conn, id)
}

pub async fn list_pending_policy_actions(
    handle: &RepositoryHandle,
) -> Result<Vec<PolicyActionRow>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    policy::list_pending_approval(&conn)
}

pub async fn find_resumable_action(
    handle: &RepositoryHandle,
    target_identity_json: &str,
    target_start_time: i64,
) -> Result<Option<PolicyActionRow>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    policy::find_resumable_action(&conn, target_identity_json, target_start_time)
}

pub async fn insert_tuning_plan(
    handle: &RepositoryHandle,
    new: NewTuningPlan,
) -> Result<TuningPlanRow, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::TuningPlanInsert {
            new,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn mark_tuning_plan_completed(
    handle: &RepositoryHandle,
    id: i64,
    status: impl Into<String>,
    completed_at: chrono::DateTime<chrono::Utc>,
    candidates_json: impl Into<String>,
) -> Result<TuningPlanRow, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::TuningPlanMarkCompleted {
            id,
            status: status.into(),
            completed_at,
            candidates_json: candidates_json.into(),
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn list_recent_tuning_plans(
    handle: &RepositoryHandle,
) -> Result<Vec<TuningPlanRow>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    tuning::list_recent(&conn)
}

pub async fn get_tuning_plan(
    handle: &RepositoryHandle,
    id: i64,
) -> Result<Option<TuningPlanRow>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    tuning::get_by_id(&conn, id)
}

pub async fn has_improved_benchmark_history(
    handle: &RepositoryHandle,
    action_type: &str,
) -> Result<bool, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    benchmark::query_improved_run_exists(&conn, action_type)
}

#[allow(clippy::type_complexity)]
pub async fn record_security_event(
    handle: &RepositoryHandle,
    new: NewSecurityEvent,
) -> Result<Option<(SecurityEventRow, SecurityIncidentRow)>, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordSecurityEvent {
            new,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

pub async fn record_security_suppression(
    handle: &RepositoryHandle,
    new: NewSecuritySuppression,
) -> Result<(), RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordSuppression {
            new,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U55: returns the previous setting for `name`, if any (`None`
/// means this is the first observation, a baseline seed rather than a
/// change) — the caller (`security::detect_guc_change`) decides
/// whether that constitutes a real, fireable event.
pub async fn record_guc_value(
    handle: &RepositoryHandle,
    name: impl Into<String>,
    setting: impl Into<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Option<String>, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordGucValue {
            name: name.into(),
            setting: setting.into(),
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U56: returns the previous `rolsuper` flag for `rolname`, if
/// any (`None` means this is the first observation, a baseline seed
/// rather than a change) — the caller (`security::detect_role_
/// superuser_granted`) decides whether that constitutes a real,
/// fireable event.
pub async fn record_role_superuser_flag(
    handle: &RepositoryHandle,
    rolname: impl Into<String>,
    rolsuper: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Option<bool>, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordRoleSuperuserFlag {
            rolname: rolname.into(),
            rolsuper,
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U57: returns whether `client_addr` was already known before
/// this call — the caller (`security::detect_unusual_client_address`)
/// decides whether that constitutes a real, fireable event.
pub async fn record_client_address_seen(
    handle: &RepositoryHandle,
    client_addr: impl Into<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordClientAddressSeen {
            client_addr: client_addr.into(),
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U58: returns whether the exact `(member, granted_role)` pair
/// was already known before this call — the caller (`security::
/// detect_role_membership_granted`) decides whether that constitutes
/// a real, fireable event.
pub async fn record_role_membership_seen(
    handle: &RepositoryHandle,
    member: impl Into<String>,
    granted_role: impl Into<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordRoleMembershipSeen {
            member: member.into(),
            granted_role: granted_role.into(),
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U59: returns whether the exact `(grantee, schema, table,
/// privilege_type)` tuple was already known before this call — the
/// caller (`security::detect_table_privilege_granted`) decides
/// whether that constitutes a real, fireable event.
#[allow(clippy::too_many_arguments)]
pub async fn record_table_privilege_grant_seen(
    handle: &RepositoryHandle,
    grantee: impl Into<String>,
    schema: impl Into<String>,
    table: impl Into<String>,
    privilege_type: impl Into<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordTablePrivilegeGrantSeen {
            grantee: grantee.into(),
            schema: schema.into(),
            table: table.into(),
            privilege_type: privilege_type.into(),
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U74 (ADR 0063's own named gap): forgets any `(member,
/// granted_role)` pair not present in `current`, the complete,
/// freshly-polled membership set for this sweep tick — so a real
/// revoke followed by a genuine re-grant fires again instead of
/// staying silently suppressed forever. Returns how many stale pairs
/// were forgotten.
pub async fn reconcile_known_role_memberships(
    handle: &RepositoryHandle,
    current: Vec<(String, String)>,
) -> Result<usize, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::ReconcileRoleMemberships {
            current,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U74 (ADR 0064's own named gap, mirrors
/// `reconcile_known_role_memberships` exactly): forgets any `(grantee,
/// schema, table, privilege_type)` tuple not present in `current`.
/// Returns how many stale tuples were forgotten.
pub async fn reconcile_known_table_privilege_grants(
    handle: &RepositoryHandle,
    current: Vec<(String, String, String, String)>,
) -> Result<usize, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::ReconcileTablePrivilegeGrants {
            current,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Unit U60 (SRS FR-DBSEC-001(a)): how far into `log_file_path` the
/// authentication-failure detector has already tailed, if ever. A
/// plain read (like `get_action`/`get_tuning_plan`), not a
/// `WriteCommand` — no write happens here.
pub async fn get_log_tail_offset(
    handle: &RepositoryHandle,
    log_file_path: &str,
) -> Result<Option<i64>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    log_tail_offset::get_offset(&conn, log_file_path)
}

/// Unit U60 (SRS FR-DBSEC-001(a)): persists the new end-of-file byte
/// offset for `log_file_path` after a real tail pass
/// (`core::dbms::log_tail::read_new_auth_failures`).
pub async fn set_log_tail_offset(
    handle: &RepositoryHandle,
    log_file_path: impl Into<String>,
    byte_offset: i64,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::SetLogTailOffset {
            log_file_path: log_file_path.into(),
            byte_offset,
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// Returns whether `identifier` was already known before this call.
pub async fn record_device_seen(
    handle: &RepositoryHandle,
    identifier: impl Into<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::RecordDeviceSeen {
            identifier: identifier.into(),
            now,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)?
}

/// SRS FR-SEC-003's own query, exposed directly (not via a `WriteCommand`
/// — reads go through the pool, same as `get_action`/`get_tuning_plan`).
pub async fn resource_is_security_flagged(
    handle: &RepositoryHandle,
    resource_descriptor_json: &str,
) -> Result<bool, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    security::resource_is_flagged(&conn, resource_descriptor_json)
}

pub async fn latest_audit_event_type(
    handle: &RepositoryHandle,
) -> Result<Option<String>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    audit::latest_event_type(&conn)
}

pub async fn list_open_security_incidents(
    handle: &RepositoryHandle,
) -> Result<Vec<(SecurityIncidentRow, i64)>, RepositoryError> {
    let conn = reader::checkout(&handle.read_pool)?;
    security::list_open_incidents(&conn)
}

pub async fn shutdown(handle: &RepositoryHandle) -> Result<(), RepositoryError> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .command_tx
        .send(WriteCommand::Shutdown { reply: reply_tx })
        .await
        .map_err(|_| RepositoryError::WriterUnavailable)?;
    reply_rx
        .await
        .map_err(|_| RepositoryError::WriterDidNotReply)
}
