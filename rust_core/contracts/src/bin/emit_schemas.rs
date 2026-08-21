//! Regenerates `/schemas/*.schema.json` from the types in this crate. CI's
//! schema-drift gate runs this and fails the build if the working tree
//! changes as a result — see docs/adr/0002-contract-first-schema-codegen.md.
//!
//! Run via `cargo run -p contracts --bin emit-schemas` from anywhere in the
//! workspace.

use contracts::telemetry::{
    CpuHistoryPoint, CpuSnapshot, DeviceSnapshot, HistoryResponse, MemoryHistoryPoint,
    MemorySnapshot, MetricValue, NetworkHistoryPoint, NetworkSnapshot, ProcessSnapshot,
    StorageHistoryPoint, StorageSnapshot, SystemStatusResponse,
};
use contracts::{
    ActionProposalOutcome, ApiError, Capability, CorrelationResult, Envelope, GatedValue, GucValue,
    HealthResponse, HostVerdict, IndexStat, LockEdge, PendingActionSummary, QueryStat,
    ReplicationStatus, ResumableActionSummary, SecurityIncidentSummary, SessionInfo, TableStat,
    TuningPlanOutcome, TuningPlanSummary,
};
use schemars::schema_for;
use std::path::PathBuf;

fn schemas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

fn write_schema<T: schemars::JsonSchema>(name: &str) -> anyhow::Result<()> {
    let schema = schema_for!(T);
    let json = serde_json::to_string_pretty(&schema)? + "\n";
    let path = schemas_dir().join(format!("{name}.schema.json"));
    std::fs::write(&path, json)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    std::fs::create_dir_all(schemas_dir())?;
    write_schema::<HealthResponse>("health_response")?;
    write_schema::<Envelope<HealthResponse>>("envelope_health_response")?;
    write_schema::<ApiError>("api_error")?;
    write_schema::<Capability>("capability")?;

    write_schema::<CpuSnapshot>("cpu_snapshot")?;
    write_schema::<Envelope<CpuSnapshot>>("envelope_cpu_snapshot")?;
    write_schema::<MemorySnapshot>("memory_snapshot")?;
    write_schema::<Envelope<MemorySnapshot>>("envelope_memory_snapshot")?;
    write_schema::<StorageSnapshot>("storage_snapshot")?;
    write_schema::<Envelope<StorageSnapshot>>("envelope_storage_snapshot")?;
    write_schema::<NetworkSnapshot>("network_snapshot")?;
    write_schema::<Envelope<NetworkSnapshot>>("envelope_network_snapshot")?;
    write_schema::<ProcessSnapshot>("process_snapshot")?;
    write_schema::<Envelope<ProcessSnapshot>>("envelope_process_snapshot")?;
    write_schema::<DeviceSnapshot>("device_snapshot")?;
    write_schema::<Envelope<DeviceSnapshot>>("envelope_device_snapshot")?;
    write_schema::<SystemStatusResponse>("system_status_response")?;
    write_schema::<Envelope<SystemStatusResponse>>("envelope_system_status_response")?;
    write_schema::<MetricValue<f64>>("metric_value_f64")?;
    write_schema::<MetricValue<u64>>("metric_value_u64")?;
    write_schema::<MetricValue<String>>("metric_value_string")?;

    write_schema::<PendingActionSummary>("pending_action_summary")?;
    write_schema::<Envelope<Vec<PendingActionSummary>>>("envelope_pending_action_summary_list")?;
    write_schema::<ActionProposalOutcome>("action_proposal_outcome")?;
    write_schema::<Envelope<ActionProposalOutcome>>("envelope_action_proposal_outcome")?;
    write_schema::<ResumableActionSummary>("resumable_action_summary")?;
    write_schema::<Envelope<Vec<ResumableActionSummary>>>(
        "envelope_resumable_action_summary_list",
    )?;
    write_schema::<TuningPlanSummary>("tuning_plan_summary")?;
    write_schema::<Envelope<Vec<TuningPlanSummary>>>("envelope_tuning_plan_summary_list")?;
    write_schema::<TuningPlanOutcome>("tuning_plan_outcome")?;
    write_schema::<Envelope<TuningPlanOutcome>>("envelope_tuning_plan_outcome")?;
    write_schema::<SecurityIncidentSummary>("security_incident_summary")?;
    write_schema::<Envelope<Vec<SecurityIncidentSummary>>>(
        "envelope_security_incident_summary_list",
    )?;
    write_schema::<HostVerdict>("host_verdict")?;
    write_schema::<Envelope<HostVerdict>>("envelope_host_verdict")?;
    write_schema::<CorrelationResult>("correlation_result")?;
    write_schema::<Envelope<CorrelationResult>>("envelope_correlation_result")?;

    write_schema::<SessionInfo>("session_info")?;
    write_schema::<Envelope<Vec<SessionInfo>>>("envelope_session_info_list")?;
    write_schema::<LockEdge>("lock_edge")?;
    write_schema::<Envelope<Vec<LockEdge>>>("envelope_lock_edge_list")?;
    write_schema::<QueryStat>("query_stat")?;
    write_schema::<GatedValue<Vec<QueryStat>>>("gated_value_query_stat_list")?;
    write_schema::<Envelope<GatedValue<Vec<QueryStat>>>>("envelope_gated_value_query_stat_list")?;
    write_schema::<TableStat>("table_stat")?;
    write_schema::<Envelope<Vec<TableStat>>>("envelope_table_stat_list")?;
    write_schema::<IndexStat>("index_stat")?;
    write_schema::<Envelope<Vec<IndexStat>>>("envelope_index_stat_list")?;
    write_schema::<ReplicationStatus>("replication_status")?;
    write_schema::<Envelope<ReplicationStatus>>("envelope_replication_status")?;
    write_schema::<GucValue>("guc_value")?;
    write_schema::<Envelope<Vec<GucValue>>>("envelope_guc_value_list")?;

    write_schema::<HistoryResponse<CpuHistoryPoint>>("history_response_cpu")?;
    write_schema::<Envelope<HistoryResponse<CpuHistoryPoint>>>("envelope_history_response_cpu")?;
    write_schema::<HistoryResponse<MemoryHistoryPoint>>("history_response_memory")?;
    write_schema::<Envelope<HistoryResponse<MemoryHistoryPoint>>>(
        "envelope_history_response_memory",
    )?;
    write_schema::<HistoryResponse<StorageHistoryPoint>>("history_response_storage")?;
    write_schema::<Envelope<HistoryResponse<StorageHistoryPoint>>>(
        "envelope_history_response_storage",
    )?;
    write_schema::<HistoryResponse<NetworkHistoryPoint>>("history_response_network")?;
    write_schema::<Envelope<HistoryResponse<NetworkHistoryPoint>>>(
        "envelope_history_response_network",
    )?;

    Ok(())
}
