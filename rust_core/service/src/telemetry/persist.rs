//! Maps the live `HostTelemetrySnapshot` to the row shape
//! `core::repository` persists. Lives here (not in `core`) so
//! `core::repository` stays decoupled from the live telemetry contract
//! types — it only ever sees `TelemetrySnapshotRow`.

use ai_ops_core::repository::TelemetrySnapshotRow;
use contracts::telemetry::MetricValue;
use serde::Serialize;

use super::HostTelemetrySnapshot;

fn value_and_state<T: Clone>(mv: &MetricValue<T>) -> (Option<T>, &'static str) {
    match mv {
        MetricValue::Supported { value } => (Some(value.clone()), "SUPPORTED"),
        MetricValue::SampleGap { .. } => (None, "SAMPLE_GAP"),
        MetricValue::CounterReset { .. } => (None, "COUNTER_RESET"),
        MetricValue::Unavailable { .. } => (None, "UNAVAILABLE"),
    }
}

fn value_only<T: Clone>(mv: &MetricValue<T>) -> Option<T> {
    value_and_state(mv).0
}

/// Sums only the `Supported` components (matching the rollup queries' own
/// "ignore non-supported samples" convention); `None` only when *none* of
/// the components were supported, never a fabricated `0`.
fn sum_supported(values: impl Iterator<Item = MetricValue<f64>>) -> Option<f64> {
    let mut sum = 0.0;
    let mut any_supported = false;
    for v in values {
        if let MetricValue::Supported { value } = v {
            sum += value;
            any_supported = true;
        }
    }
    any_supported.then_some(sum)
}

/// Renders an enum via its own `Serialize` impl (`SCREAMING_SNAKE_CASE`,
/// matching the wire format) rather than `{:?}` — the two aren't the same
/// casing, and the DB/history-query code expects the wire casing.
fn enum_to_db_string<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

pub fn snapshot_to_row(snapshot: &HostTelemetrySnapshot) -> TelemetrySnapshotRow {
    let cpu = &snapshot.cpu;
    let (cpu_aggregate_util_pct, cpu_aggregate_util_state) =
        value_and_state(&cpu.aggregate_utilization_percent);
    let cpu_load_avg_1m = value_only(&cpu.load_average_1m);
    let cpu_per_core_json = serde_json::to_string(&cpu.per_core_utilization_percent).ok();

    let mem = &snapshot.memory;
    let (mem_used_bytes, mem_used_bytes_state) = value_and_state(&mem.used_bytes);

    let storage = &snapshot.storage;
    let storage_read_bytes_ps =
        sum_supported(storage.volumes.iter().map(|v| v.read_bytes_per_sec.clone()));
    let storage_write_bytes_ps = sum_supported(
        storage
            .volumes
            .iter()
            .map(|v| v.write_bytes_per_sec.clone()),
    );
    let storage_volumes_json = serde_json::to_string(&storage.volumes).ok();

    let network = &snapshot.network;
    let net_rx_bytes_ps = sum_supported(
        network
            .interfaces
            .iter()
            .map(|i| i.rx_bytes_per_sec.clone()),
    );
    let net_tx_bytes_ps = sum_supported(
        network
            .interfaces
            .iter()
            .map(|i| i.tx_bytes_per_sec.clone()),
    );
    let net_interfaces_json = serde_json::to_string(&network.interfaces).ok();

    TelemetrySnapshotRow {
        ts: cpu.timestamp,
        cpu_aggregate_util_pct,
        cpu_aggregate_util_state: cpu_aggregate_util_state.to_string(),
        cpu_load_avg_1m,
        cpu_pressure: enum_to_db_string(&cpu.pressure),
        cpu_per_core_json,
        mem_total_bytes: value_only(&mem.total_bytes).map(|v| v as i64),
        mem_used_bytes: mem_used_bytes.map(|v| v as i64),
        mem_used_bytes_state: mem_used_bytes_state.to_string(),
        mem_available_bytes: value_only(&mem.available_bytes).map(|v| v as i64),
        mem_swap_used_bytes: value_only(&mem.swap_used_bytes).map(|v| v as i64),
        mem_pressure: enum_to_db_string(&mem.pressure),
        storage_read_bytes_ps,
        storage_write_bytes_ps,
        storage_volumes_json,
        net_rx_bytes_ps,
        net_tx_bytes_ps,
        net_interfaces_json,
        process_total_count: Some(i64::from(snapshot.processes.total_count)),
        device_count: Some(snapshot.devices.devices.len() as i64),
        containerized: cpu.containerized || mem.containerized,
    }
}
