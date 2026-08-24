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
use super::client_address_history;
use super::device_trust;
use super::error::RepositoryError;
use super::guc_history;
use super::log_tail_offset;
use super::models::{
    AuditEventRecorded, BenchmarkRunRow, NewAuditEvent, NewBenchmarkRun, NewProposedAction,
    NewSecurityEvent, NewSecuritySuppression, NewTuningPlan, PerformanceAnalysisRow,
    PolicyActionRow, RetentionReport, SecurityEventRow, SecurityIncidentRow, TelemetrySnapshotRow,
    TransitionPatch, TuningPlanRow,
};
use super::performance_analysis::insert_performance_analysis;
use super::policy::{insert_proposed_action, transition};
use super::retention::run_sweep;
use super::role_history;
use super::role_membership_history;
use super::security::{insert_suppression, record_event_checked};
use super::table_privilege_history;
use super::telemetry_store::insert_raw_snapshot;
use super::tuning::{insert_tuning_plan, mark_plan_completed};

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
    /// Inserts a new `tuning_plans` row (unit U10), `IN_FLIGHT` status. A
    /// second insert for a target that already has an `IN_FLIGHT` row hits
    /// the partial `UNIQUE` index and comes back as
    /// `RepositoryError::TuningPlanAlreadyInFlight` (FR-TUNE-002).
    TuningPlanInsert {
        new: NewTuningPlan,
        reply: oneshot::Sender<Result<TuningPlanRow, RepositoryError>>,
    },
    /// Marks a `tuning_plans` row `COMPLETED`/`REJECTED`, recording the
    /// final candidate list (each candidate's outcome, unit U10).
    TuningPlanMarkCompleted {
        id: i64,
        status: String,
        completed_at: DateTime<Utc>,
        candidates_json: String,
        reply: oneshot::Sender<Result<TuningPlanRow, RepositoryError>>,
    },
    /// Records a detected security event (unit U11), correlating it into
    /// an existing `OPEN` incident or opening a new one — `Ok(None)` means
    /// it was suppressed (FR-SEC-005), not an error.
    #[allow(clippy::type_complexity)]
    RecordSecurityEvent {
        new: NewSecurityEvent,
        reply: oneshot::Sender<
            Result<Option<(SecurityEventRow, SecurityIncidentRow)>, RepositoryError>,
        >,
    },
    /// Records a new suppression rule (unit U11, FR-SEC-005).
    RecordSuppression {
        new: NewSecuritySuppression,
        reply: oneshot::Sender<Result<(), RepositoryError>>,
    },
    /// Records that a device identifier has been observed (unit U11) —
    /// replaces the service sampler's own ephemeral in-process `HashSet`.
    /// Replies with whether it was *already* known before this call.
    RecordDeviceSeen {
        identifier: String,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<bool, RepositoryError>>,
    },
    /// Records a GUC's polled setting (unit U55, SRS FR-DBSEC-001(e)),
    /// replying with its previous setting, if any.
    RecordGucValue {
        name: String,
        setting: String,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<Option<String>, RepositoryError>>,
    },
    /// Records a role's polled `rolsuper` flag (unit U56, SRS
    /// FR-DBSEC-001(b)), replying with its previous flag, if any.
    RecordRoleSuperuserFlag {
        rolname: String,
        rolsuper: bool,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<Option<bool>, RepositoryError>>,
    },
    /// Records that a client address has been observed (unit U57, SRS
    /// FR-DBSEC-001(d)), replying with whether it was *already* known.
    RecordClientAddressSeen {
        client_addr: String,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<bool, RepositoryError>>,
    },
    /// Records that a `(member, granted_role)` pair has been observed
    /// (unit U58, SRS FR-DBSEC-001(b)'s deferred remainder), replying
    /// with whether it was *already* known.
    RecordRoleMembershipSeen {
        member: String,
        granted_role: String,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<bool, RepositoryError>>,
    },
    /// Records that a `(grantee, schema, table, privilege_type)` tuple
    /// has been observed (unit U59, SRS FR-DBSEC-001(c)), replying
    /// with whether it was *already* known.
    #[allow(clippy::type_complexity)]
    RecordTablePrivilegeGrantSeen {
        grantee: String,
        schema: String,
        table: String,
        privilege_type: String,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<bool, RepositoryError>>,
    },
    /// Forgets any `(member, granted_role)` pair not in `current` —
    /// unit U74, ADR 0063's own named revocation-tracking gap.
    ReconcileRoleMemberships {
        current: Vec<(String, String)>,
        reply: oneshot::Sender<Result<usize, RepositoryError>>,
    },
    /// Forgets any `(grantee, schema, table, privilege_type)` tuple not
    /// in `current` — unit U74, ADR 0064's own named revocation-
    /// tracking gap.
    ReconcileTablePrivilegeGrants {
        current: Vec<(String, String, String, String)>,
        reply: oneshot::Sender<Result<usize, RepositoryError>>,
    },
    /// Persists the new end-of-file byte offset for a log file after a
    /// real tail pass (unit U60, SRS FR-DBSEC-001(a)).
    SetLogTailOffset {
        log_file_path: String,
        byte_offset: i64,
        now: DateTime<Utc>,
        reply: oneshot::Sender<Result<(), RepositoryError>>,
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
                    WriteCommand::TuningPlanInsert { new, reply } => {
                        drop(reply.send(insert_tuning_plan(&conn, &new)));
                    }
                    WriteCommand::TuningPlanMarkCompleted {
                        id,
                        status,
                        completed_at,
                        candidates_json,
                        reply,
                    } => {
                        let result = mark_plan_completed(
                            &conn,
                            id,
                            &status,
                            completed_at,
                            &candidates_json,
                        );
                        drop(reply.send(result));
                    }
                    WriteCommand::RecordSecurityEvent { new, reply } => {
                        drop(reply.send(record_event_checked(&conn, &new)));
                    }
                    WriteCommand::RecordSuppression { new, reply } => {
                        drop(reply.send(insert_suppression(&conn, &new)));
                    }
                    WriteCommand::RecordDeviceSeen {
                        identifier,
                        now,
                        reply,
                    } => {
                        drop(reply.send(device_trust::record_seen(&conn, &identifier, now)));
                    }
                    WriteCommand::RecordGucValue {
                        name,
                        setting,
                        now,
                        reply,
                    } => {
                        drop(reply.send(guc_history::record_guc_value(
                            &conn, &name, &setting, now,
                        )));
                    }
                    WriteCommand::RecordRoleSuperuserFlag {
                        rolname,
                        rolsuper,
                        now,
                        reply,
                    } => {
                        drop(reply.send(role_history::record_role_superuser_flag(
                            &conn, &rolname, rolsuper, now,
                        )));
                    }
                    WriteCommand::RecordClientAddressSeen {
                        client_addr,
                        now,
                        reply,
                    } => {
                        drop(reply.send(client_address_history::record_seen(
                            &conn,
                            &client_addr,
                            now,
                        )));
                    }
                    WriteCommand::RecordRoleMembershipSeen {
                        member,
                        granted_role,
                        now,
                        reply,
                    } => {
                        drop(reply.send(role_membership_history::record_seen(
                            &conn,
                            &member,
                            &granted_role,
                            now,
                        )));
                    }
                    WriteCommand::RecordTablePrivilegeGrantSeen {
                        grantee,
                        schema,
                        table,
                        privilege_type,
                        now,
                        reply,
                    } => {
                        drop(reply.send(table_privilege_history::record_seen(
                            &conn,
                            &grantee,
                            &schema,
                            &table,
                            &privilege_type,
                            now,
                        )));
                    }
                    WriteCommand::ReconcileRoleMemberships { current, reply } => {
                        drop(reply.send(role_membership_history::reconcile(&conn, &current)));
                    }
                    WriteCommand::ReconcileTablePrivilegeGrants { current, reply } => {
                        drop(reply.send(table_privilege_history::reconcile(&conn, &current)));
                    }
                    WriteCommand::SetLogTailOffset {
                        log_file_path,
                        byte_offset,
                        now,
                        reply,
                    } => {
                        drop(reply.send(log_tail_offset::set_offset(
                            &conn,
                            &log_file_path,
                            byte_offset,
                            now,
                        )));
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
