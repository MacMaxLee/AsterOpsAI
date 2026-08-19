//! Background host telemetry sampler, mirroring `self_metrics.rs`'s idiom:
//! an immediate first tick, then a variable-duration loop writing the
//! latest snapshot into a shared `Arc<RwLock<_>>` — no live I/O in any
//! request path.

use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::HostTelemetrySnapshot;

#[cfg(target_os = "linux")]
const NORMAL_INTERVAL: Duration = Duration::from_secs(1);
/// Requirement 8: back host telemetry sampling off when the host is itself
/// under CPU pressure, so the act of observing doesn't compound the problem
/// (SRS FR-SYS-003).
#[cfg(target_os = "linux")]
const BACKED_OFF_INTERVAL: Duration = Duration::from_secs(5);

/// Process/device enumeration is the dominant per-tick cost (hundreds of
/// `/proc/[pid]/*` reads, vs. a handful of reads for everything else
/// combined — measured: it's what pushed the service over the DoD's <2%
/// idle CPU budget when refreshed every tick). Neither needs 1s freshness,
/// so they're only re-parsed every Nth fast tick; ticks in between reuse the
/// previous snapshot.
#[cfg(target_os = "linux")]
const PROCESS_DEVICE_REFRESH_EVERY_N_TICKS: u64 = 5;

/// Persisting every 1s tick would mean 604,800 raw rows per family over the
/// 7-day retention window — real volume, decoupled here from the API-facing
/// 1s live-snapshot cadence, which is unaffected (unit U2 requirement 2).
#[cfg(target_os = "linux")]
const PERSIST_EVERY_N_TICKS: u64 = 10;

#[cfg(target_os = "linux")]
mod linux_impl {
    use std::collections::HashSet;

    use ai_ops_core::telemetry::{
        build_system_status_response, parse_cpu_snapshot, parse_devices, parse_memory_snapshot,
        parse_network_snapshot, parse_process_snapshot, parse_storage_snapshot, PrevCpuState,
        PrevDiskState, PrevNetState, PrevProcessState, SampleContext,
    };
    use contracts::telemetry::{
        CpuPressure, DeviceInfo, DeviceKind, DeviceSnapshot, MetricValue, ProcessSnapshot,
    };
    use contracts::{Capability, CapabilityFamily};
    use platform::linux::{volume_capacity, RealProcSource};

    use super::*;

    pub struct HostTelemetrySampler {
        source: RealProcSource,
        last_instant: Option<Instant>,
        prev_cpu: Option<PrevCpuState>,
        prev_net: Option<PrevNetState>,
        prev_disk: Option<PrevDiskState>,
        prev_processes: PrevProcessState,
        known_devices: HashSet<String>,
        tick_count: u64,
        last_processes: Option<ProcessSnapshot>,
        last_devices: Option<DeviceSnapshot>,
        pub interval: Duration,
    }

    impl HostTelemetrySampler {
        pub fn new() -> Self {
            Self {
                source: RealProcSource,
                last_instant: None,
                prev_cpu: None,
                prev_net: None,
                prev_disk: None,
                prev_processes: PrevProcessState::new(),
                known_devices: HashSet::new(),
                tick_count: 0,
                last_processes: None,
                last_devices: None,
                interval: NORMAL_INTERVAL,
            }
        }

        pub fn tick(&mut self) -> HostTelemetrySnapshot {
            let now_instant = Instant::now();
            let elapsed = self
                .last_instant
                .map(|prev| now_instant.duration_since(prev))
                .unwrap_or(self.interval);
            self.last_instant = Some(now_instant);

            let ctx = SampleContext {
                now: chrono::Utc::now(),
                elapsed,
                configured_interval: self.interval,
            };

            let cpu = match parse_cpu_snapshot(&self.source, &ctx, self.prev_cpu.as_ref()) {
                Ok((snapshot, prev)) => {
                    self.prev_cpu = Some(prev);
                    snapshot
                }
                Err(err) => {
                    tracing::error!(error = %err, "failed to parse cpu telemetry");
                    self.prev_cpu = None;
                    HostTelemetrySnapshot::unavailable(&err.to_string()).cpu
                }
            };

            let memory = match parse_memory_snapshot(&self.source, &ctx) {
                Ok(snapshot) => snapshot,
                Err(err) => {
                    tracing::error!(error = %err, "failed to parse memory telemetry");
                    HostTelemetrySnapshot::unavailable(&err.to_string()).memory
                }
            };

            let mut storage =
                match parse_storage_snapshot(&self.source, &ctx, self.prev_disk.as_ref()) {
                    Ok((snapshot, prev)) => {
                        self.prev_disk = Some(prev);
                        snapshot
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to parse storage telemetry");
                        self.prev_disk = None;
                        HostTelemetrySnapshot::unavailable(&err.to_string()).storage
                    }
                };
            for volume in &mut storage.volumes {
                match volume_capacity(&volume.mount_point) {
                    Ok(capacity) => {
                        volume.capacity_bytes = MetricValue::Supported {
                            value: capacity.capacity_bytes,
                        };
                        volume.free_bytes = MetricValue::Supported {
                            value: capacity.free_bytes,
                        };
                        volume.available_bytes = MetricValue::Supported {
                            value: capacity.available_bytes,
                        };
                    }
                    Err(err) => {
                        let reason = err.to_string();
                        volume.capacity_bytes = MetricValue::Unavailable {
                            reason: reason.clone(),
                        };
                        volume.free_bytes = MetricValue::Unavailable {
                            reason: reason.clone(),
                        };
                        volume.available_bytes = MetricValue::Unavailable { reason };
                    }
                }
            }

            let network = match parse_network_snapshot(&self.source, &ctx, self.prev_net.as_ref()) {
                Ok((snapshot, prev)) => {
                    self.prev_net = Some(prev);
                    snapshot
                }
                Err(err) => {
                    tracing::error!(error = %err, "failed to parse network telemetry");
                    self.prev_net = None;
                    HostTelemetrySnapshot::unavailable(&err.to_string()).network
                }
            };

            let refresh_processes_and_devices = self
                .tick_count
                .is_multiple_of(PROCESS_DEVICE_REFRESH_EVERY_N_TICKS);
            self.tick_count += 1;

            let processes = match (refresh_processes_and_devices, &self.last_processes) {
                (false, Some(previous)) => previous.clone(),
                _ => {
                    let snapshot = match parse_process_snapshot(
                        &self.source,
                        &ctx,
                        Some(&self.prev_processes),
                    ) {
                        Ok((snapshot, prev)) => {
                            self.prev_processes = prev;
                            snapshot
                        }
                        Err(err) => {
                            tracing::error!(error = %err, "failed to parse process telemetry");
                            self.prev_processes = PrevProcessState::new();
                            HostTelemetrySnapshot::unavailable(&err.to_string()).processes
                        }
                    };
                    self.last_processes = Some(snapshot.clone());
                    snapshot
                }
            };

            let devices = match (refresh_processes_and_devices, &self.last_devices) {
                (false, Some(previous)) => previous.clone(),
                _ => {
                    let snapshot = match parse_devices(&self.source) {
                        Ok(candidates) => {
                            let devices = candidates
                                .into_iter()
                                .map(|candidate| {
                                    let trusted =
                                        !self.known_devices.insert(candidate.identifier.clone());
                                    DeviceInfo {
                                        identifier: candidate.identifier,
                                        kind: if candidate.removable {
                                            DeviceKind::RemovableStorage
                                        } else {
                                            DeviceKind::BlockStorage
                                        },
                                        name: candidate.name,
                                        removable: candidate.removable,
                                        trusted,
                                    }
                                })
                                .collect();
                            DeviceSnapshot {
                                timestamp: ctx.now,
                                devices,
                            }
                        }
                        Err(err) => {
                            tracing::error!(error = %err, "failed to parse device telemetry");
                            HostTelemetrySnapshot::unavailable(&err.to_string()).devices
                        }
                    };
                    self.last_devices = Some(snapshot.clone());
                    snapshot
                }
            };

            let cpu_pressure = cpu.pressure;
            let memory_pressure = memory.pressure;
            let containerized = cpu.containerized || memory.containerized;

            self.interval =
                if cpu_pressure == CpuPressure::High || cpu_pressure == CpuPressure::Critical {
                    BACKED_OFF_INTERVAL
                } else {
                    NORMAL_INTERVAL
                };

            let mut capabilities = contracts::default_capability_registry();
            capabilities.insert(
                CapabilityFamily::SelfMetrics,
                Capability::Unavailable {
                    reason: "reported on /health, not /system/status".to_string(),
                },
            );

            let system_status = match build_system_status_response(
                &self.source,
                &ctx,
                containerized,
                cpu_pressure,
                memory_pressure,
                self.interval.as_millis() as u64,
                capabilities,
            ) {
                Ok(response) => response,
                Err(err) => {
                    tracing::error!(error = %err, "failed to build system status");
                    HostTelemetrySnapshot::unavailable(&err.to_string()).system_status
                }
            };

            HostTelemetrySnapshot {
                cpu,
                memory,
                storage,
                network,
                processes,
                devices,
                system_status,
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub fn spawn(
    _platform: Arc<dyn platform::PlatformAdapter>,
    repository: Option<ai_ops_core::repository::RepositoryHandle>,
) -> Arc<RwLock<HostTelemetrySnapshot>> {
    let mut sampler = linux_impl::HostTelemetrySampler::new();
    let first = sampler.tick();
    if let Some(repo) = &repository {
        ai_ops_core::repository::try_persist_telemetry_snapshot(
            repo,
            super::persist::snapshot_to_row(&first),
        );
    }
    let snapshot = Arc::new(RwLock::new(first));

    let shared = snapshot.clone();
    tokio::spawn(async move {
        let mut tick_count: u64 = 0;
        loop {
            tokio::time::sleep(sampler.interval).await;
            let value = sampler.tick();

            tick_count += 1;
            if let Some(repo) = &repository {
                if tick_count.is_multiple_of(PERSIST_EVERY_N_TICKS) {
                    ai_ops_core::repository::try_persist_telemetry_snapshot(
                        repo,
                        super::persist::snapshot_to_row(&value),
                    );
                }
            }

            *shared.write().await = value;
        }
    });

    snapshot
}

#[cfg(not(target_os = "linux"))]
pub fn spawn(
    _platform: Arc<dyn platform::PlatformAdapter>,
    _repository: Option<ai_ops_core::repository::RepositoryHandle>,
) -> Arc<RwLock<HostTelemetrySnapshot>> {
    Arc::new(RwLock::new(HostTelemetrySnapshot::unavailable(
        "host telemetry not implemented on this platform yet, see unit U12",
    )))
}
