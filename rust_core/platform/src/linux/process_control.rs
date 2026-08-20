//! Unit U8's two real actions: process priority and CPU affinity. Narrow,
//! typed wrappers — mirrors `linux/mod.rs`'s own `get_rusage_self` pattern
//! (`#[allow(unsafe_code)]` per function, a `# SAFETY:` comment per
//! `unsafe` block).
//!
//! `getpriority(2)`'s return value is ambiguous on its own (`-1` is both a
//! legitimate nice value and the error sentinel — POSIX requires clearing
//! `errno` first and checking it after). Rather than take on that
//! errno-clearing dance, the *getters* here read `/proc/[pid]/stat`'s own
//! `nice` field (19) and `/proc/[pid]/status`'s `Cpus_allowed_list`
//! instead — real, unambiguous, and consistent with how the rest of this
//! codebase already prefers `/proc` parsing over syscalls where both give
//! the same answer (`core::telemetry`'s whole design). Only the *setters*
//! (`setpriority`/`sched_setaffinity`) need a real `unsafe` syscall — both
//! have unambiguous `0`/`-1` return values, no errno-clearing needed.

use std::collections::BTreeSet;

use crate::adapter::{CpuAffinityMask, ProcessPriority};
use crate::error::CapabilityError;

fn map_last_os_error() -> CapabilityError {
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EPERM) => CapabilityError::PermissionRequired(err.to_string()),
        Some(libc::ESRCH) => CapabilityError::Unavailable(format!("no such process: {err}")),
        _ => CapabilityError::Io(err),
    }
}

fn read_stat_field(pid: u32, overall_field: usize) -> Result<String, CapabilityError> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|_| CapabilityError::Unavailable(format!("no such process: pid {pid}")))?;
    let close = raw
        .rfind(')')
        .ok_or_else(|| CapabilityError::Unavailable(format!("malformed /proc/{pid}/stat")))?;
    let rest: Vec<&str> = raw[close + 1..].split_whitespace().collect();
    // pid=1, comm=2, state=3 (rest[0]), ... nice=19 -> rest[19-3].
    rest.get(overall_field - 3)
        .map(|s| s.to_string())
        .ok_or_else(|| CapabilityError::Unavailable(format!("malformed /proc/{pid}/stat")))
}

fn nice_to_priority(nice: i32) -> ProcessPriority {
    if nice >= 15 {
        ProcessPriority::Idle
    } else if nice >= 5 {
        ProcessPriority::BelowNormal
    } else if nice > -5 {
        ProcessPriority::Normal
    } else if nice > -10 {
        ProcessPriority::AboveNormal
    } else {
        ProcessPriority::High
    }
}

fn priority_to_nice(priority: ProcessPriority) -> i32 {
    match priority {
        ProcessPriority::Idle => 19,
        ProcessPriority::BelowNormal => 10,
        ProcessPriority::Normal => 0,
        ProcessPriority::AboveNormal => -5,
        ProcessPriority::High => -10,
    }
}

pub fn get_priority(pid: u32) -> Result<ProcessPriority, CapabilityError> {
    let raw = read_stat_field(pid, 19)?;
    let nice: i32 = raw.parse().map_err(|_| {
        CapabilityError::Unavailable(format!("unparsable nice field {raw:?} for pid {pid}"))
    })?;
    Ok(nice_to_priority(nice))
}

/// # SAFETY (documented at the call site below)
/// `setpriority(2)`'s `0`/`-1` return is unambiguous — no errno-clearing
/// needed, unlike `getpriority`.
#[allow(unsafe_code)]
pub fn set_priority(pid: u32, priority: ProcessPriority) -> Result<(), CapabilityError> {
    let nice = priority_to_nice(priority);
    // SAFETY: `pid`/`nice` are plain integers; `setpriority` performs no
    // pointer dereference on this process's memory.
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };
    if rc != 0 {
        return Err(map_last_os_error());
    }
    Ok(())
}

fn parse_cpu_list(spec: &str) -> Result<BTreeSet<usize>, CapabilityError> {
    let mut cpus = BTreeSet::new();
    for part in spec.trim().split(',').filter(|p| !p.is_empty()) {
        if let Some((start, end)) = part.split_once('-') {
            let start: usize = start
                .parse()
                .map_err(|_| CapabilityError::Unavailable(format!("bad cpu range {part:?}")))?;
            let end: usize = end
                .parse()
                .map_err(|_| CapabilityError::Unavailable(format!("bad cpu range {part:?}")))?;
            cpus.extend(start..=end);
        } else {
            let cpu: usize = part
                .parse()
                .map_err(|_| CapabilityError::Unavailable(format!("bad cpu index {part:?}")))?;
            cpus.insert(cpu);
        }
    }
    Ok(cpus)
}

pub fn get_cpu_affinity(pid: u32) -> Result<CpuAffinityMask, CapabilityError> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .map_err(|_| CapabilityError::Unavailable(format!("no such process: pid {pid}")))?;
    let line = raw
        .lines()
        .find(|l| l.starts_with("Cpus_allowed_list:"))
        .ok_or_else(|| {
            CapabilityError::Unavailable(format!("no Cpus_allowed_list in /proc/{pid}/status"))
        })?;
    let spec = line.trim_start_matches("Cpus_allowed_list:").trim();
    Ok(CpuAffinityMask {
        cpus: parse_cpu_list(spec)?,
    })
}

/// # SAFETY (documented at the call site below)
#[allow(unsafe_code)]
pub fn set_cpu_affinity(pid: u32, mask: &CpuAffinityMask) -> Result<(), CapabilityError> {
    if mask.cpus.is_empty() {
        return Err(CapabilityError::Unavailable(
            "a CPU affinity mask must name at least one CPU".to_string(),
        ));
    }
    // SAFETY: `set` is a plain POD C struct; `CPU_ZERO`/`CPU_SET` only ever
    // write within its own bounds via libc's own safe-shaped helpers.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_ZERO(&mut set);
    }
    for &cpu in &mask.cpus {
        if cpu >= libc::CPU_SETSIZE as usize {
            return Err(CapabilityError::Unavailable(format!(
                "cpu index {cpu} exceeds this platform's CPU_SETSIZE ({})",
                libc::CPU_SETSIZE
            )));
        }
        // SAFETY: `cpu < CPU_SETSIZE`, checked just above; `set` is a
        // valid, correctly-sized, exclusively-owned local.
        unsafe {
            libc::CPU_SET(cpu, &mut set);
        }
    }
    // SAFETY: `pid` is a plain integer; `&set` points to a valid,
    // correctly-sized `cpu_set_t` for the duration of this call.
    let rc = unsafe {
        libc::sched_setaffinity(
            pid as libc::pid_t,
            std::mem::size_of::<libc::cpu_set_t>(),
            &set,
        )
    };
    if rc != 0 {
        return Err(map_last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Read-only against this test binary's own process — safe to run
    // alongside other tests in the same process (no scheduling/affinity
    // mutation here). The setters are exercised against a real, disposable
    // *spawned* target process instead, in
    // core/tests/actions_host_priority_affinity_test.rs — mutating this
    // shared test binary's own priority/affinity would affect every other
    // test thread running concurrently in it.

    #[test]
    fn get_priority_of_self_is_a_real_value() {
        let pid = std::process::id();
        let priority = get_priority(pid).expect("get_priority on self");
        // A freshly-started `cargo test` process is normally Normal; this
        // just proves the /proc parse round-trips into a real variant, not
        // a specific value the test environment might not guarantee.
        let _ = priority;
    }

    #[test]
    fn get_cpu_affinity_of_self_is_non_empty() {
        let pid = std::process::id();
        let mask = get_cpu_affinity(pid).expect("get_cpu_affinity on self");
        assert!(
            !mask.cpus.is_empty(),
            "a running process always has at least one allowed CPU"
        );
    }

    #[test]
    fn getters_report_no_such_process_for_an_impossible_pid() {
        // PID 1 always exists (init); a very large pid essentially never
        // does on a real system without exhausting the pid space first.
        let impossible_pid = 4_000_000_000u32;
        assert!(get_priority(impossible_pid).is_err());
        assert!(get_cpu_affinity(impossible_pid).is_err());
    }

    #[test]
    fn nice_to_priority_and_back_round_trip_at_named_points() {
        for priority in [
            ProcessPriority::Idle,
            ProcessPriority::BelowNormal,
            ProcessPriority::Normal,
            ProcessPriority::AboveNormal,
            ProcessPriority::High,
        ] {
            let nice = priority_to_nice(priority);
            assert_eq!(nice_to_priority(nice), priority);
        }
    }

    #[test]
    fn parse_cpu_list_handles_ranges_and_singletons() {
        let parsed = parse_cpu_list("0-2,5,7-8").expect("parse");
        assert_eq!(parsed, BTreeSet::from([0, 1, 2, 5, 7, 8]));
    }
}
