//! macOS CPU telemetry via Mach `host_statistics64` API.
//!
//! Collects system-wide and per-core CPU utilization using:
//! - `host_statistics64(HOST_CPU_LOAD_INFO)` for aggregate ticks
//! - `host_processor_info(PROCESSOR_CPU_LOAD_INFO)` for per-core ticks
//! - `getloadavg()` for POSIX load averages
//!
//! Mirrors `telemetry/cpu.rs` structure but uses macOS-specific APIs.
//! Established in unit U95.

use contracts::telemetry::{CpuPressure, CpuSnapshot, MetricValue};

use super::context::SampleContext;
use super::error::TelemetryError;
use super::rate::utilization_percent;

// ============================================================================
// FFI Bindings for Mach APIs
// ============================================================================

// CPU state indices for cpu_ticks array
const CPU_STATE_USER: usize = 0;
const CPU_STATE_SYSTEM: usize = 1;
const CPU_STATE_IDLE: usize = 2;
const CPU_STATE_NICE: usize = 3;
const CPU_STATE_MAX: usize = 4;

// Mach API constants
const HOST_CPU_LOAD_INFO: i32 = 3;
const HOST_CPU_LOAD_INFO_COUNT: u32 = 4;
const PROCESSOR_CPU_LOAD_INFO: i32 = 2;
const KERN_SUCCESS: i32 = 0;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct host_cpu_load_info {
    cpu_ticks: [u32; CPU_STATE_MAX],
}

#[link(name = "c")]
extern "C" {
    fn mach_host_self() -> u32; // mach_port_t
    fn mach_task_self() -> u32; // mach_port_t

    fn host_statistics64(
        host_priv: u32,
        flavor: i32,
        host_info_out: *mut i32,
        host_info_outCnt: *mut u32,
    ) -> i32;

    fn host_processor_info(
        host: u32,
        flavor: i32,
        out_processor_count: *mut u32,
        out_processor_info: *mut *mut i32,
        out_processor_infoCnt: *mut u32,
    ) -> i32;

    fn vm_deallocate(target_task: u32, address: usize, size: usize) -> i32;

    fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
}

// ============================================================================
// State Tracking Structures
// ============================================================================

#[derive(Debug, Clone, Copy, Default)]
struct TicksSample {
    idle: u64,
    total: u64,
}

#[derive(Debug, Clone)]
pub struct PrevCpuState {
    aggregate: TicksSample,
    per_core: Vec<TicksSample>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Converts host_cpu_load_info ticks to TicksSample.
fn ticks_from_cpu_load_info(info: &host_cpu_load_info) -> TicksSample {
    let user = info.cpu_ticks[CPU_STATE_USER] as u64;
    let system = info.cpu_ticks[CPU_STATE_SYSTEM] as u64;
    let idle = info.cpu_ticks[CPU_STATE_IDLE] as u64;
    let nice = info.cpu_ticks[CPU_STATE_NICE] as u64;

    TicksSample {
        idle,
        total: user + system + idle + nice,
    }
}

/// Gets aggregate CPU ticks via host_statistics64(HOST_CPU_LOAD_INFO).
///
/// # SAFETY (documented at call site below)
/// host_statistics64 operates on a Mach port and a fixed-size output buffer.
#[allow(unsafe_code)]
fn get_aggregate_cpu_ticks() -> Result<TicksSample, TelemetryError> {
    let mut info = host_cpu_load_info {
        cpu_ticks: [0; CPU_STATE_MAX],
    };
    let mut count = HOST_CPU_LOAD_INFO_COUNT;

    // SAFETY: `info` is a valid, correctly-sized writable buffer;
    // `host_statistics64` only ever writes to it and returns a status code
    // we check before treating it as initialized. `mach_host_self()` returns
    // a well-known Mach port that doesn't need deallocation.
    let rc = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_CPU_LOAD_INFO,
            &mut info as *mut host_cpu_load_info as *mut i32,
            &mut count,
        )
    };

    if rc != KERN_SUCCESS {
        return Err(TelemetryError::Io {
            path: "host_statistics64(HOST_CPU_LOAD_INFO)".to_string(),
            source: std::io::Error::from_raw_os_error(rc),
        });
    }

    Ok(ticks_from_cpu_load_info(&info))
}

/// Gets per-core CPU ticks via host_processor_info(PROCESSOR_CPU_LOAD_INFO).
///
/// # SAFETY (documented at call site below)
/// host_processor_info allocates memory via Mach that must be deallocated.
#[allow(unsafe_code)]
fn get_per_core_cpu_ticks() -> Result<Vec<TicksSample>, TelemetryError> {
    let mut processor_count: u32 = 0;
    let mut processor_info: *mut i32 = std::ptr::null_mut();
    let mut processor_info_count: u32 = 0;

    // SAFETY: `processor_count`, `processor_info`, and `processor_info_count`
    // are valid output pointers that `host_processor_info` writes to. The
    // returned `processor_info` buffer is allocated by Mach and must be
    // explicitly deallocated via `vm_deallocate` below. `mach_host_self()`
    // returns a well-known Mach port that doesn't need deallocation.
    let rc = unsafe {
        host_processor_info(
            mach_host_self(),
            PROCESSOR_CPU_LOAD_INFO,
            &mut processor_count,
            &mut processor_info,
            &mut processor_info_count,
        )
    };

    if rc != KERN_SUCCESS {
        return Err(TelemetryError::Io {
            path: "host_processor_info(PROCESSOR_CPU_LOAD_INFO)".to_string(),
            source: std::io::Error::from_raw_os_error(rc),
        });
    }

    // SAFETY: `processor_info` points to a Mach-allocated buffer containing
    // `processor_count * CPU_STATE_MAX` i32 values. We iterate through it
    // safely by treating it as a slice, then deallocate below.
    let per_core = unsafe {
        let total_elements = (processor_count as usize) * CPU_STATE_MAX;
        let slice = std::slice::from_raw_parts(processor_info, total_elements);

        let mut cores = Vec::with_capacity(processor_count as usize);
        for i in 0..processor_count as usize {
            let base = i * CPU_STATE_MAX;
            let user = slice[base + CPU_STATE_USER] as u64;
            let system = slice[base + CPU_STATE_SYSTEM] as u64;
            let idle = slice[base + CPU_STATE_IDLE] as u64;
            let nice = slice[base + CPU_STATE_NICE] as u64;

            cores.push(TicksSample {
                idle,
                total: user + system + idle + nice,
            });
        }
        cores
    };

    // SAFETY: `processor_info` was allocated by `host_processor_info` and
    // must be deallocated via `vm_deallocate`. The size is
    // `processor_info_count * sizeof(integer_t)` where integer_t is i32.
    unsafe {
        let size = processor_info_count as usize * std::mem::size_of::<i32>();
        vm_deallocate(mach_task_self(), processor_info as usize, size);
    }

    Ok(per_core)
}

/// Gets POSIX load averages via getloadavg(3).
fn get_load_averages() -> Result<(f64, f64, f64), TelemetryError> {
    let mut loadavg: [f64; 3] = [0.0; 3];

    // SAFETY: `loadavg` is a valid, correctly-sized writable buffer;
    // `getloadavg` is a standard POSIX function that writes at most `nelem`
    // f64 values to the buffer.
    #[allow(unsafe_code)]
    let rc = unsafe { getloadavg(loadavg.as_mut_ptr(), 3) };

    if rc < 3 {
        return Err(TelemetryError::Io {
            path: "getloadavg".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    Ok((loadavg[0], loadavg[1], loadavg[2]))
}

/// Classifies CPU pressure based on aggregate utilization percent.
/// Mirrors `telemetry/pressure.rs::classify_cpu_pressure` thresholds.
fn classify_cpu_pressure(aggregate_utilization_percent: f64) -> CpuPressure {
    if aggregate_utilization_percent >= 95.0 {
        CpuPressure::Critical
    } else if aggregate_utilization_percent >= 85.0 {
        CpuPressure::High
    } else if aggregate_utilization_percent >= 70.0 {
        CpuPressure::Elevated
    } else {
        CpuPressure::Normal
    }
}

// ============================================================================
// Main Parser Function
// ============================================================================

/// Parses a CPU snapshot on macOS using Mach APIs.
///
/// Returns `(CpuSnapshot, PrevCpuState)` where the second element is the
/// state to pass to the next invocation for delta calculations.
///
/// On the first sample (`prev == None`), rate-based metrics return
/// `MetricValue::Unavailable` since there's no baseline for deltas.
pub fn parse_cpu_snapshot(
    ctx: &SampleContext,
    prev: Option<&PrevCpuState>,
) -> Result<(CpuSnapshot, PrevCpuState), TelemetryError> {
    // Collect current measurements
    let aggregate = get_aggregate_cpu_ticks()?;
    let per_core = get_per_core_cpu_ticks()?;
    let (load_1, load_5, load_15) = get_load_averages()?;

    let logical_core_count = per_core.len() as u32;

    // Calculate utilization from deltas (or mark unavailable on first sample)
    let (aggregate_utilization_percent, per_core_utilization_percent) = match prev {
        Some(prev) => {
            let agg_pct = utilization_percent(
                prev.aggregate.idle,
                aggregate.idle,
                prev.aggregate.total,
                aggregate.total,
                ctx,
            );

            let per_core_pct: Vec<MetricValue<f64>> = per_core
                .iter()
                .enumerate()
                .map(|(i, sample)| match prev.per_core.get(i) {
                    Some(prev_sample) => utilization_percent(
                        prev_sample.idle,
                        sample.idle,
                        prev_sample.total,
                        sample.total,
                        ctx,
                    ),
                    None => MetricValue::Unavailable {
                        reason: "insufficient samples yet".to_string(),
                    },
                })
                .collect();

            (agg_pct, per_core_pct)
        }
        None => {
            let unavailable = || MetricValue::Unavailable {
                reason: "insufficient samples yet".to_string(),
            };
            let per_core_pct = per_core.iter().map(|_| unavailable()).collect();
            (unavailable(), per_core_pct)
        }
    };

    // CPU frequency: macOS doesn't expose per-core frequency easily
    let frequency_mhz = (0..logical_core_count)
        .map(|_| MetricValue::Unavailable {
            reason: "macOS does not expose per-core CPU frequency".to_string(),
        })
        .collect();

    // Context switches and interrupts: macOS doesn't expose these counters
    let context_switches_per_sec = MetricValue::Unavailable {
        reason: "macOS does not expose context switch counters".to_string(),
    };
    let interrupts_per_sec = MetricValue::Unavailable {
        reason: "macOS does not expose interrupt counters".to_string(),
    };

    // Pressure classification
    let pressure = match &aggregate_utilization_percent {
        MetricValue::Supported { value } => classify_cpu_pressure(*value),
        _ => CpuPressure::Normal,
    };

    // macOS doesn't typically use cgroups
    let containerized = false;

    let snapshot = CpuSnapshot {
        timestamp: ctx.now,
        logical_core_count,
        aggregate_utilization_percent,
        per_core_utilization_percent,
        frequency_mhz,
        load_average_1m: MetricValue::Supported { value: load_1 },
        load_average_5m: MetricValue::Supported { value: load_5 },
        load_average_15m: MetricValue::Supported { value: load_15 },
        context_switches_per_sec,
        interrupts_per_sec,
        pressure,
        containerized,
    };

    let next_prev = PrevCpuState {
        aggregate,
        per_core,
    };

    Ok((snapshot, next_prev))
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn fixed_ctx(elapsed_secs: u64) -> SampleContext {
        SampleContext {
            now: chrono::Utc::now(),
            elapsed: Duration::from_secs(elapsed_secs),
            configured_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn first_sample_returns_unavailable_for_rates() {
        let (snapshot, _prev) = parse_cpu_snapshot(&fixed_ctx(1), None)
            .expect("first sample should succeed");

        // Rate-based metrics should be Unavailable on first sample
        assert!(
            matches!(
                snapshot.aggregate_utilization_percent,
                MetricValue::Unavailable { .. }
            ),
            "aggregate utilization should be unavailable on first sample"
        );

        // All per-core metrics should also be Unavailable
        for (i, core_pct) in snapshot.per_core_utilization_percent.iter().enumerate() {
            assert!(
                matches!(core_pct, MetricValue::Unavailable { .. }),
                "core {} utilization should be unavailable on first sample",
                i
            );
        }

        // Load averages should be Supported (they're gauges, not rates)
        assert!(
            matches!(snapshot.load_average_1m, MetricValue::Supported { .. }),
            "load average should be supported"
        );
    }

    #[test]
    fn second_sample_calculates_utilization() {
        let (_, prev) = parse_cpu_snapshot(&fixed_ctx(1), None)
            .expect("first sample should succeed");

        // Wait a bit and take second sample (500ms to ensure CPU ticks change)
        std::thread::sleep(Duration::from_millis(500));

        let (snapshot, _) = parse_cpu_snapshot(&fixed_ctx(1), Some(&prev))
            .expect("second sample should succeed");

        // Utilization should now be Supported
        match snapshot.aggregate_utilization_percent {
            MetricValue::Supported { value } => {
                assert!(
                    value >= 0.0 && value <= 100.0,
                    "utilization should be 0-100%, got {}",
                    value
                );
            }
            other => panic!(
                "aggregate utilization should be Supported on second sample, got {:?}",
                other
            ),
        }

        // Load averages should still be Supported
        assert!(matches!(
            snapshot.load_average_1m,
            MetricValue::Supported { .. }
        ));
    }

    #[test]
    fn core_count_matches_system() {
        let (snapshot, _) = parse_cpu_snapshot(&fixed_ctx(1), None)
            .expect("sample should succeed");

        // Verify core count is reasonable (at least 1, at most 128)
        assert!(
            snapshot.logical_core_count > 0 && snapshot.logical_core_count <= 128,
            "logical core count should be reasonable, got {}",
            snapshot.logical_core_count
        );

        // Verify per-core vector length matches core count
        assert_eq!(
            snapshot.per_core_utilization_percent.len(),
            snapshot.logical_core_count as usize,
            "per-core vector length should match core count"
        );
    }

    #[test]
    fn unavailable_metrics_documented() {
        let (snapshot, _) = parse_cpu_snapshot(&fixed_ctx(1), None)
            .expect("sample should succeed");

        // CPU frequency should be unavailable on macOS
        for freq in &snapshot.frequency_mhz {
            assert!(
                matches!(freq, MetricValue::Unavailable { .. }),
                "CPU frequency should be unavailable on macOS"
            );
        }

        // Context switches should be unavailable
        assert!(
            matches!(
                snapshot.context_switches_per_sec,
                MetricValue::Unavailable { .. }
            ),
            "context switches should be unavailable on macOS"
        );

        // Interrupts should be unavailable
        assert!(
            matches!(snapshot.interrupts_per_sec, MetricValue::Unavailable { .. }),
            "interrupts should be unavailable on macOS"
        );
    }

    #[test]
    fn not_containerized() {
        let (snapshot, _) = parse_cpu_snapshot(&fixed_ctx(1), None)
            .expect("sample should succeed");

        // macOS doesn't typically use cgroups
        assert!(!snapshot.containerized, "should not be containerized");
    }
}
