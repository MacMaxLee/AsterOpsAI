//! SQLite persistence: bounded-growth telemetry history and a
//! tamper-evident audit log (unit U2). See docs/TRS.md §12-14 and
//! docs/adr/0007-sqlite-single-writer-task-no-writer-pool.md.

pub mod audit;
pub mod connection;
pub mod error;
pub mod history;
pub mod migrations;
pub mod models;
pub mod reader;
pub mod retention;
pub mod telemetry_store;
mod time;
pub mod writer;

use std::path::PathBuf;

pub use connection::ReadPool;
pub use error::RepositoryError;
pub use history::{
    query_cpu_history, query_memory_history, query_network_history, query_storage_history,
    HistoryRange, ResolvedRange,
};
pub use models::{
    AuditEventRecorded, ChainVerification, NewAuditEvent, RetentionAuditDetail, RetentionReport,
    TelemetrySnapshotRow,
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
