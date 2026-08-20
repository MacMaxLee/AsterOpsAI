//! SQLite persistence: bounded-growth telemetry history and a
//! tamper-evident audit log (unit U2). See docs/TRS.md §12-14 and
//! docs/adr/0007-sqlite-single-writer-task-no-writer-pool.md.

pub mod audit;
pub mod benchmark;
pub mod connection;
pub mod device_trust;
pub mod error;
pub mod history;
pub mod migrations;
pub mod models;
pub mod performance_analysis;
pub mod policy;
pub mod reader;
pub mod retention;
pub mod security;
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
