use rusqlite::{params, Connection};

use super::error::RepositoryError;
use super::models::TelemetrySnapshotRow;
use super::time::format_ts;

/// Inserts one persisted sampler tick. The mapping from the live
/// `HostTelemetrySnapshot` (a `service`-crate/`contracts`-typed value) to
/// this row lives in `service::telemetry::sampler`, not here — this crate
/// only knows about the row shape, keeping `core::repository` decoupled
/// from the live telemetry contract types.
pub fn insert_raw_snapshot(
    conn: &Connection,
    row: &TelemetrySnapshotRow,
) -> Result<(), RepositoryError> {
    conn.execute(
        "INSERT INTO telemetry_snapshots (
            ts,
            cpu_aggregate_util_pct, cpu_aggregate_util_state, cpu_load_avg_1m, cpu_pressure, cpu_per_core_json,
            mem_total_bytes, mem_used_bytes, mem_used_bytes_state, mem_available_bytes, mem_swap_used_bytes, mem_pressure,
            storage_read_bytes_ps, storage_write_bytes_ps, storage_volumes_json,
            net_rx_bytes_ps, net_tx_bytes_ps, net_interfaces_json,
            process_total_count, device_count, containerized
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
        )",
        params![
            format_ts(row.ts),
            row.cpu_aggregate_util_pct,
            row.cpu_aggregate_util_state,
            row.cpu_load_avg_1m,
            row.cpu_pressure,
            row.cpu_per_core_json,
            row.mem_total_bytes,
            row.mem_used_bytes,
            row.mem_used_bytes_state,
            row.mem_available_bytes,
            row.mem_swap_used_bytes,
            row.mem_pressure,
            row.storage_read_bytes_ps,
            row.storage_write_bytes_ps,
            row.storage_volumes_json,
            row.net_rx_bytes_ps,
            row.net_tx_bytes_ps,
            row.net_interfaces_json,
            row.process_total_count,
            row.device_count,
            i64::from(row.containerized),
        ],
    )?;
    Ok(())
}
