//! Regenerates `/schemas/*.schema.json` from the types in this crate. CI's
//! schema-drift gate runs this and fails the build if the working tree
//! changes as a result — see docs/adr/0002-contract-first-schema-codegen.md.
//!
//! Run via `cargo run -p contracts --bin emit-schemas` from anywhere in the
//! workspace.

use contracts::telemetry::{
    CpuSnapshot, DeviceSnapshot, MemorySnapshot, MetricValue, NetworkSnapshot, ProcessSnapshot,
    StorageSnapshot, SystemStatusResponse,
};
use contracts::{ApiError, Capability, Envelope, HealthResponse};
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

    Ok(())
}
