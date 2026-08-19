//! Host telemetry, aggregated into one snapshot the seven `/api/v1/*`
//! telemetry handlers read from (unit U1). Plain `contracts`-typed data —
//! not itself platform-gated; only [`sampler::spawn`] has a real (Linux) and
//! a stub (everywhere else) implementation.

pub mod persist;
pub mod sampler;

use std::collections::BTreeMap;

use chrono::Utc;
use contracts::telemetry::{
    CpuPressure, CpuSnapshot, DeviceSnapshot, MemoryPressure, MemorySnapshot, MetricValue,
    NetworkSnapshot, ProcessSnapshot, StorageSnapshot, SystemStatusResponse,
};

#[derive(Debug, Clone)]
pub struct HostTelemetrySnapshot {
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub storage: StorageSnapshot,
    pub network: NetworkSnapshot,
    pub processes: ProcessSnapshot,
    pub devices: DeviceSnapshot,
    pub system_status: SystemStatusResponse,
}

impl HostTelemetrySnapshot {
    /// Used before the first real sample lands, and as the permanent value
    /// on platforms without a host telemetry implementation yet (see
    /// `sampler`'s non-Linux stub). `/system/status`'s `capabilities` map is
    /// the authoritative signal that these values shouldn't be trusted —
    /// this constructor exists so every other field still has *some* valid,
    /// honestly-`Unavailable` value rather than the response failing outright.
    pub fn unavailable(reason: &str) -> Self {
        let now = Utc::now();
        fn unavailable<T>(reason: &str) -> MetricValue<T> {
            MetricValue::Unavailable {
                reason: reason.to_string(),
            }
        }

        Self {
            cpu: CpuSnapshot {
                timestamp: now,
                logical_core_count: 0,
                aggregate_utilization_percent: unavailable(reason),
                per_core_utilization_percent: Vec::new(),
                frequency_mhz: Vec::new(),
                load_average_1m: unavailable(reason),
                load_average_5m: unavailable(reason),
                load_average_15m: unavailable(reason),
                context_switches_per_sec: unavailable(reason),
                interrupts_per_sec: unavailable(reason),
                // No `Unavailable` variant on this enum (see the U1 plan's
                // note); the capabilities map is what tells a client not to
                // trust this value on platforms that hit this constructor.
                pressure: CpuPressure::Normal,
                containerized: false,
            },
            memory: MemorySnapshot {
                timestamp: now,
                total_bytes: unavailable(reason),
                used_bytes: unavailable(reason),
                available_bytes: unavailable(reason),
                cached_bytes: unavailable(reason),
                buffers_bytes: unavailable(reason),
                swap_total_bytes: unavailable(reason),
                swap_used_bytes: unavailable(reason),
                swap_free_bytes: unavailable(reason),
                pressure: MemoryPressure::Normal,
                containerized: false,
                numa_nodes: unavailable(reason),
            },
            storage: StorageSnapshot {
                timestamp: now,
                volumes: Vec::new(),
            },
            network: NetworkSnapshot {
                timestamp: now,
                interfaces: Vec::new(),
            },
            processes: ProcessSnapshot {
                timestamp: now,
                total_count: 0,
                processes: Vec::new(),
            },
            devices: DeviceSnapshot {
                timestamp: now,
                devices: Vec::new(),
            },
            system_status: SystemStatusResponse {
                timestamp: now,
                uptime_seconds: 0,
                containerized: false,
                cpu_pressure: CpuPressure::Normal,
                memory_pressure: MemoryPressure::Normal,
                sample_interval_ms: 0,
                capabilities: BTreeMap::new(),
            },
        }
    }
}
