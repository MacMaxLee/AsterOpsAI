//! The single dedicated writer: one real OS thread owns the write
//! `Connection` for the service's whole lifetime, fed by an mpsc channel —
//! never a writer pool. rusqlite is synchronous; a persistent actor thread
//! is the idiomatic way to combine a blocking resource with async callers
//! (vs. bouncing every write through `spawn_blocking`). See ADR 0007.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

use super::audit::{insert_audit_event, seed_chain_state};
use super::benchmark::{insert_benchmark_run, mark_rolled_back};
use super::error::RepositoryError;
use super::models::{
    AuditEventRecorded, BenchmarkRunRow, NewAuditEvent, NewBenchmarkRun, NewProposedAction,
    PerformanceAnalysisRow, PolicyActionRow, RetentionReport, TelemetrySnapshotRow,
    TransitionPatch,
};
use super::performance_analysis::insert_performance_analysis;
use super::policy::{insert_proposed_action, transition};
use super::retention::run_sweep;
use super::telemetry_store::insert_raw_snapshot;

const COMMAND_CHANNEL_CAPACITY: usize = 1024;

pub enum WriteCommand {
    /// Fire-and-forget: sent via `try_send` from the sampler tick, never
    /// blocks. A full channel means the sample is silently dropped
    /// (logged), matching the "slow consumer must not block sampling" rule
    /// already applied to the API server and the sampler itself.
    InsertTelemetrySnapshot(Box<TelemetrySnapshotRow>),
    /// Fire-and-forget, same reasoning as `InsertTelemetrySnapshot` — a
    /// dropped performance-analysis result under channel pressure is
    /// logged, never blocks the caller (unit U5).
    InsertPerformanceAnalysis(Box<PerformanceAnalysisRow>),
    /// Low-frequency; has a reply since callers may want the resulting
    /// id/hash back.
    InsertAuditEvent {
        new: NewAuditEvent,
        reply: oneshot::Sender<Result<AuditEventRecorded, RepositoryError>>,
    },
    /// `now` is caller-supplied rather than read internally — production
    /// callers pass `Utc::now()`, tests pass simulated timestamps (see the
    /// 30-day simulated fill test), avoiding both clock drift between
    /// enqueue and processing and any need for a test-only code path.
    RunRetentionSweep {
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<RetentionReport, RepositoryError>>,
    },
    /// Proposes a new action-lifecycle row, or a rollback attempt of a
    /// previously executed one (unit U7). Has a reply since every caller
    /// needs the assigned row id back.
    PolicyPropose {
        new: NewProposedAction,
        reply: oneshot::Sender<Result<PolicyActionRow, RepositoryError>>,
    },
    /// The one atomic compare-and-swap lifecycle transition every policy
    /// step (grant, authorize/consume, record result, start rollback) goes
    /// through (unit U7) — see `repository::policy::transition`'s own doc
    /// comment for why this is race-free without extra locking.
    #[allow(clippy::type_complexity)]
    PolicyTransition {
        id: i64,
        expected_status: String,
        new_status: String,
        now: DateTime<Utc>,
        check_not_expired: bool,
        patch: TransitionPatch,
        reply: oneshot::Sender<Result<PolicyActionRow, RepositoryError>>,
    },
    /// Inserts a new `benchmark_runs` row (unit U9) — a completed run
    /// (verdict known) or a `BASELINE_UNSTABLE` abort record. Has a reply
    /// since the caller needs the assigned row id back.
    BenchmarkRunInsert {
        new: NewBenchmarkRun,
        reply: oneshot::Sender<Result<BenchmarkRunRow, RepositoryError>>,
    },
    /// Marks a benchmark run as rolled back, recording which rollback
    /// action row did it (TRS §35).
    BenchmarkRunMarkRolledBack {
        id: i64,
        rollback_action_id: i64,
        reply: oneshot::Sender<Result<BenchmarkRunRow, RepositoryError>>,
    },
    /// Lets tests (and, later, graceful process shutdown) deterministically
    /// drain the channel and join the thread instead of racing thread exit
    /// against process exit.
    Shutdown { reply: oneshot::Sender<()> },
}

pub type CommandSender = mpsc::Sender<WriteCommand>;

/// Spawns the writer thread against an already-open, already-migrated
/// connection. Seeds the audit chain's `next_id`/`last_row_hash` state from
/// the database once, up front — single writer thread means this is the
/// only place those are ever read back; every subsequent insert updates
/// them locally with no query and no race.
pub fn spawn(
    conn: Connection,
) -> Result<(CommandSender, std::thread::JoinHandle<()>), RepositoryError> {
    let (tx, mut rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);

    let (mut next_audit_id, mut last_row_hash) = seed_chain_state(&conn)?;

    let handle = std::thread::Builder::new()
        .name("aoai-db-writer".to_string())
        .spawn(move || {
            while let Some(cmd) = rx.blocking_recv() {
                match cmd {
                    WriteCommand::InsertTelemetrySnapshot(row) => {
                        if let Err(err) = insert_raw_snapshot(&conn, &row) {
                            tracing::warn!(error = %err, "failed to persist telemetry snapshot");
                        }
                    }
                    WriteCommand::InsertPerformanceAnalysis(row) => {
                        if let Err(err) = insert_performance_analysis(&conn, &row) {
                            tracing::warn!(error = %err, "failed to persist performance analysis result");
                        }
                    }
                    WriteCommand::InsertAuditEvent { new, reply } => {
                        let id = next_audit_id;
                        let result = insert_audit_event(&conn, id, &last_row_hash, &new);
                        if let Ok(recorded) = &result {
                            next_audit_id += 1;
                            last_row_hash = recorded.row_hash.clone();
                        }
                        drop(reply.send(result));
                    }
                    WriteCommand::RunRetentionSweep { now, reply } => {
                        let result = run_sweep(&conn, &mut next_audit_id, &mut last_row_hash, now);
                        drop(reply.send(result));
                    }
                    WriteCommand::PolicyPropose { new, reply } => {
                        drop(reply.send(insert_proposed_action(&conn, &new)));
                    }
                    WriteCommand::PolicyTransition {
                        id,
                        expected_status,
                        new_status,
                        now,
                        check_not_expired,
                        patch,
                        reply,
                    } => {
                        let result = transition(
                            &conn,
                            id,
                            &expected_status,
                            &new_status,
                            now,
                            check_not_expired,
                            &patch,
                        );
                        drop(reply.send(result));
                    }
                    WriteCommand::BenchmarkRunInsert { new, reply } => {
                        drop(reply.send(insert_benchmark_run(&conn, &new)));
                    }
                    WriteCommand::BenchmarkRunMarkRolledBack {
                        id,
                        rollback_action_id,
                        reply,
                    } => {
                        drop(reply.send(mark_rolled_back(&conn, id, rollback_action_id)));
                    }
                    WriteCommand::Shutdown { reply } => {
                        let _ = reply.send(());
                        break;
                    }
                }
            }
        })?;

    Ok((tx, handle))
}
