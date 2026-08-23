//! SRS FR-SYS-003 (unit U65): sampling backs off from `NORMAL_INTERVAL`
//! (1s) to `BACKED_OFF_INTERVAL` (5s) once `tick()` observes real
//! `CpuPressure::High`/`Critical`. `HostTelemetrySampler` previously
//! hardcoded `RealProcSource`, so this was implemented but unverified —
//! there was no way to drive a controlled fixture through `tick()`
//! without depending on the live machine's actual CPU load. Now
//! injectable the same way `self_metrics::spawn_with_interval` (unit
//! U62) made its own interval testable.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use contracts::telemetry::CpuPressure;
use platform::linux::ProcSource;
use service::telemetry::sampler::linux_impl::HostTelemetrySampler;

/// The real `cpu-saturated` fixture (`tests/fixtures/proc/cpu-saturated/`)
/// already used by `core::telemetry::cpu`'s own tests: a `before`/`after`
/// pair where idle time barely advances relative to total ticks. Unlike
/// `FixtureProcSource` (fixed to one phase, `core`-crate-internal,
/// `#[cfg(test)]`-gated so unreachable from this crate), this switches
/// from `before` to `after` after the first `proc/stat` read — tracking
/// phase via that one counter is sufficient since `parse_cpu_snapshot`
/// always reads `proc/stat` first, exactly once per tick, before
/// `proc/loadavg`/`proc/uptime`. Every other path (memory, storage,
/// network, process, device, cgroup, frequency files) returns
/// `NotFound` — `tick()`'s own per-section error handling degrades each
/// of those to `Unavailable` independently rather than failing the
/// whole tick, and this test only asserts on the CPU-driven backoff.
struct ScriptedCpuPressureSource {
    stat_reads: AtomicUsize,
}

impl ScriptedCpuPressureSource {
    fn new() -> Self {
        Self {
            stat_reads: AtomicUsize::new(0),
        }
    }

    fn current_phase(&self) -> usize {
        self.stat_reads.load(Ordering::SeqCst).saturating_sub(1)
    }
}

const BEFORE_STAT: &str =
    include_str!("../../../tests/fixtures/proc/cpu-saturated/before/proc/stat");
const AFTER_STAT: &str = include_str!("../../../tests/fixtures/proc/cpu-saturated/after/proc/stat");
const BEFORE_LOADAVG: &str =
    include_str!("../../../tests/fixtures/proc/cpu-saturated/before/proc/loadavg");
const AFTER_LOADAVG: &str =
    include_str!("../../../tests/fixtures/proc/cpu-saturated/after/proc/loadavg");
const BEFORE_UPTIME: &str =
    include_str!("../../../tests/fixtures/proc/cpu-saturated/before/proc/uptime");
const AFTER_UPTIME: &str =
    include_str!("../../../tests/fixtures/proc/cpu-saturated/after/proc/uptime");

impl ProcSource for ScriptedCpuPressureSource {
    fn read(&self, path: &str) -> io::Result<String> {
        match path {
            "proc/stat" => {
                let phase = self.stat_reads.fetch_add(1, Ordering::SeqCst);
                Ok(if phase == 0 { BEFORE_STAT } else { AFTER_STAT }.to_string())
            }
            "proc/loadavg" => Ok(if self.current_phase() == 0 {
                BEFORE_LOADAVG
            } else {
                AFTER_LOADAVG
            }
            .to_string()),
            "proc/uptime" => Ok(if self.current_phase() == 0 {
                BEFORE_UPTIME
            } else {
                AFTER_UPTIME
            }
            .to_string()),
            _ => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }
}

#[test]
fn sampling_backs_off_once_cpu_pressure_crosses_high() {
    let mut sampler = HostTelemetrySampler::with_source(Box::new(ScriptedCpuPressureSource::new()));

    // First tick: no previous sample yet, so utilization (and therefore
    // pressure) is `Unavailable` -> `Normal` (core::telemetry::cpu's own
    // documented fallback) -> interval stays at NORMAL_INTERVAL.
    let first = sampler.tick();
    assert_eq!(first.cpu.pressure, CpuPressure::Normal);
    assert_eq!(sampler.interval, Duration::from_secs(1));

    // Second tick: a real before -> after delta, saturated enough that
    // idle time barely advances relative to total ticks.
    let second = sampler.tick();
    assert!(
        matches!(
            second.cpu.pressure,
            CpuPressure::High | CpuPressure::Critical
        ),
        "expected High or Critical pressure from the cpu-saturated fixture, got {:?}",
        second.cpu.pressure
    );
    assert_eq!(
        sampler.interval,
        Duration::from_secs(5),
        "SRS FR-SYS-003: sampling must back off once pressure crosses High"
    );
}
