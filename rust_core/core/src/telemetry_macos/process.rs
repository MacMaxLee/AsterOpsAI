//! macOS Process telemetry via libproc.
//!
//! Collects per-process statistics using:
//! - `proc_listpids()` to enumerate all processes
//! - `proc_pidinfo()` with PROC_PIDTASKINFO for CPU/memory stats
//! - `proc_pidinfo()` with PROC_PIDTBSDINFO for UID, start time, comm
//! - `proc_pidpath()` for executable path
//!
//! Mirrors `telemetry/process.rs` structure but uses macOS-specific libproc APIs.
//! Established in unit U100.

use std::collections::HashMap;

use contracts::telemetry::{MetricValue, ProcessCategory, ProcessInfo, ProcessSnapshot};
use contracts::Capability;

use super::context::SampleContext;
use super::error::TelemetryError;
use super::rate::rate_per_second;

// ============================================================================
// FFI Bindings for libproc
// ============================================================================

const PROC_PIDTASKINFO: i32 = 4;
const PROC_PIDTBSDINFO: i32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct proc_taskinfo {
    pti_virtual_size: u64,
    pti_resident_size: u64,
    pti_total_user: u64,
    pti_total_system: u64,
    pti_threads_user: u64,
    pti_threads_system: u64,
    pti_policy: i32,
    pti_faults: i32,
    pti_pageins: i32,
    pti_cow_faults: i32,
    pti_messages_sent: i32,
    pti_messages_received: i32,
    pti_syscalls_mach: i32,
    pti_syscalls_unix: i32,
    pti_csw: i32,
    pti_threadnum: i32,
    pti_numrunning: i32,
    pti_priority: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct proc_bsdinfo {
    pbi_flags: u32,
    pbi_status: u32,
    pbi_xstatus: u32,
    pbi_pid: u32,
    pbi_ppid: u32,
    pbi_uid: u32,
    pbi_gid: u32,
    pbi_ruid: u32,
    pbi_rgid: u32,
    pbi_svuid: u32,
    pbi_svgid: u32,
    pbi_reserved1: u32,
    pbi_comm: [libc::c_char; 16],
    pbi_name: [libc::c_char; 32],
    pbi_nfiles: u32,
    pbi_pgid: u32,
    pbi_pjobc: u32,
    pbi_e_tdev: u32,
    pbi_e_tpgid: u32,
    pbi_nice: i32,
    pbi_start_tvsec: u64,
    pbi_start_tvusec: u64,
}

extern "C" {
    fn proc_listpids(
        type_: u32,
        typeinfo: u32,
        buffer: *mut libc::c_void,
        buffersize: i32,
    ) -> i32;

    fn proc_pidinfo(
        pid: i32,
        flavor: i32,
        arg: u64,
        buffer: *mut libc::c_void,
        buffersize: i32,
    ) -> i32;

    fn proc_pidpath(pid: i32, buffer: *mut libc::c_void, buffersize: u32) -> i32;
}

const PROC_ALL_PIDS: u32 = 1;

// ============================================================================
// State Tracking Structures
// ============================================================================

#[derive(Debug, Clone, Copy, Default)]
pub struct PrevProcessCounters {
    cpu_time_ns: u64, // total user + system time in nanoseconds
}

pub type PrevProcessState = HashMap<u32, PrevProcessCounters>;

// ============================================================================
// Helper Functions
// ============================================================================

/// Gets list of all PIDs on the system.
///
/// # SAFETY (documented at call site below)
/// proc_listpids writes to a buffer we provide, returns byte count written.
#[allow(unsafe_code)]
fn get_pid_list() -> Result<Vec<i32>, TelemetryError> {
    // First call: get required buffer size
    // SAFETY: proc_listpids with NULL buffer and size 0 returns required size
    let size = unsafe { proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };

    if size <= 0 {
        return Err(TelemetryError::Io {
            path: "proc_listpids(size)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    // Allocate buffer for PIDs (size is in bytes, each PID is 4 bytes)
    let num_pids = size as usize / std::mem::size_of::<i32>();
    let mut pids: Vec<i32> = vec![0; num_pids];

    // Second call: get actual PIDs
    // SAFETY: `pids` is a valid, correctly-sized writable buffer.
    // proc_listpids writes PIDs to it and returns byte count written.
    let written = unsafe {
        proc_listpids(
            PROC_ALL_PIDS,
            0,
            pids.as_mut_ptr() as *mut libc::c_void,
            size,
        )
    };

    if written < 0 {
        return Err(TelemetryError::Io {
            path: "proc_listpids(read)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    // Truncate to actual number of PIDs written
    let actual_count = (written as usize) / std::mem::size_of::<i32>();
    pids.truncate(actual_count);

    // Filter out zero PIDs (unused slots)
    Ok(pids.into_iter().filter(|&pid| pid > 0).collect())
}

/// Gets task info (CPU, memory) for a process.
///
/// # SAFETY (documented at call site below)
/// proc_pidinfo writes to a buffer we provide.
#[allow(unsafe_code)]
fn get_task_info(pid: i32) -> Option<proc_taskinfo> {
    let mut info = std::mem::MaybeUninit::<proc_taskinfo>::zeroed();

    // SAFETY: `info` is a valid writable buffer of the correct size.
    // proc_pidinfo writes the task info struct to it.
    let ret = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of::<proc_taskinfo>() as i32,
        )
    };

    if ret as usize == std::mem::size_of::<proc_taskinfo>() {
        // SAFETY: proc_pidinfo successfully wrote the struct
        Some(unsafe { info.assume_init() })
    } else {
        None
    }
}

/// Gets BSD info (UID, comm, start time) for a process.
///
/// # SAFETY (documented at call site below)
/// proc_pidinfo writes to a buffer we provide.
#[allow(unsafe_code)]
fn get_bsd_info(pid: i32) -> Option<proc_bsdinfo> {
    let mut info = std::mem::MaybeUninit::<proc_bsdinfo>::zeroed();

    // SAFETY: `info` is a valid writable buffer of the correct size.
    // proc_pidinfo writes the BSD info struct to it.
    let ret = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of::<proc_bsdinfo>() as i32,
        )
    };

    if ret as usize == std::mem::size_of::<proc_bsdinfo>() {
        // SAFETY: proc_pidinfo successfully wrote the struct
        Some(unsafe { info.assume_init() })
    } else {
        None
    }
}

/// Gets executable path for a process.
///
/// # SAFETY (documented at call site below)
/// proc_pidpath writes to a buffer we provide.
#[allow(unsafe_code)]
fn get_exe_path(pid: i32) -> Option<String> {
    let mut path_buf = vec![0u8; libc::PATH_MAX as usize];

    // SAFETY: `path_buf` is a valid writable buffer.
    // proc_pidpath writes a null-terminated path to it.
    let ret = unsafe {
        proc_pidpath(
            pid,
            path_buf.as_mut_ptr() as *mut libc::c_void,
            path_buf.len() as u32,
        )
    };

    if ret > 0 {
        // Find null terminator
        let null_pos = path_buf.iter().position(|&c| c == 0).unwrap_or(ret as usize);
        String::from_utf8(path_buf[..null_pos].to_vec()).ok()
    } else {
        None
    }
}

/// Converts a C char array to a Rust String.
fn c_char_array_to_string(arr: &[libc::c_char]) -> String {
    let null_pos = arr.iter().position(|&c| c == 0).unwrap_or(arr.len());
    let bytes: Vec<u8> = arr[..null_pos].iter().map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).to_string()
}

/// Classifies a process (simplified for macOS - no cgroups).
fn classify_process_macos(comm: &str, exe_path: Option<&str>, uid: u32) -> ProcessCategory {
    const KNOWN_DBMS_COMMS: &[&str] = &["postgres", "postmaster", "mysqld", "mariadbd"];
    const KNOWN_SYSTEM_COMMS: &[&str] = &[
        "launchd",
        "kernel_task",
        "kextd",
        "UserEventAgent",
        "systemstats",
        "syslogd",
    ];

    let exe_name = exe_path.and_then(|p| p.rsplit('/').next()).unwrap_or("");

    // DBMS engines
    if KNOWN_DBMS_COMMS.contains(&comm) || KNOWN_DBMS_COMMS.contains(&exe_name) {
        return ProcessCategory::DbmsEngine;
    }

    // System processes (uid 0 or known system processes)
    if uid == 0 || KNOWN_SYSTEM_COMMS.contains(&comm) {
        return ProcessCategory::System;
    }

    // Background services (daemons typically run as specific UIDs < 500)
    if uid > 0 && uid < 500 {
        return ProcessCategory::BackgroundService;
    }

    // User applications (uid >= 500)
    if uid >= 500 {
        return ProcessCategory::UserApplication;
    }

    ProcessCategory::Unknown
}

/// Parses a single process.
fn parse_one_process(
    pid: i32,
    ctx: &SampleContext,
    prev: Option<&PrevProcessCounters>,
) -> Option<(ProcessInfo, PrevProcessCounters)> {
    let task_info = get_task_info(pid)?;
    let bsd_info = get_bsd_info(pid)?;

    let comm = c_char_array_to_string(&bsd_info.pbi_comm);
    let exe_path = get_exe_path(pid);

    // CPU time: pti_total_user and pti_total_system are in nanoseconds
    let cpu_time_ns = task_info.pti_total_user + task_info.pti_total_system;

    let cpu_percent = match prev {
        Some(p) => {
            // Calculate rate in nanoseconds per second, then convert to percentage
            let ns_per_sec = rate_per_second(p.cpu_time_ns, cpu_time_ns, ctx);
            match ns_per_sec {
                MetricValue::Supported { value } => MetricValue::Supported {
                    // 1 second = 1e9 ns, so 100% = 1e9 ns/sec
                    value: (value / 1_000_000_000.0) * 100.0,
                },
                other => other,
            }
        }
        None => MetricValue::Unavailable {
            reason: "insufficient samples yet".to_string(),
        },
    };

    // RSS in bytes (pti_resident_size is already in bytes)
    let rss_bytes = MetricValue::Supported {
        value: task_info.pti_resident_size,
    };

    // Start time: pbi_start_tvsec is seconds since epoch
    // For macOS, we use seconds as "ticks" (no fixed tick rate like Linux)
    let start_time_ticks = bsd_info.pbi_start_tvsec;

    let category = classify_process_macos(&comm, exe_path.as_deref(), bsd_info.pbi_uid);

    // cmdline: macOS doesn't easily expose this via libproc
    // Would need sysctl KERN_PROCARGS2 which is more complex
    let cmdline = MetricValue::Unavailable {
        reason: "macOS libproc does not easily expose cmdline; requires sysctl KERN_PROCARGS2"
            .to_string(),
    };

    let info = ProcessInfo {
        pid: pid as u32,
        start_time_ticks,
        comm,
        cmdline,
        owner_uid: bsd_info.pbi_uid,
        cpu_percent,
        rss_bytes,
        category,
        // Disk I/O: not exposed via libproc (would need IOKit or DTrace)
        disk_io_capability: Capability::Unavailable {
            reason: "macOS does not expose per-process disk I/O via libproc; requires IOKit or DTrace".to_string(),
        },
        disk_read_bytes_per_sec: MetricValue::Unavailable {
            reason: "not available via libproc".to_string(),
        },
        disk_write_bytes_per_sec: MetricValue::Unavailable {
            reason: "not available via libproc".to_string(),
        },
        // Network I/O: not exposed via standard APIs
        network_io_capability: Capability::Unavailable {
            reason: "macOS does not expose per-process network I/O via standard APIs".to_string(),
        },
        network_rx_bytes_per_sec: MetricValue::Unavailable {
            reason: "not available on macOS".to_string(),
        },
        network_tx_bytes_per_sec: MetricValue::Unavailable {
            reason: "not available on macOS".to_string(),
        },
    };

    let next_prev = PrevProcessCounters { cpu_time_ns };

    Some((info, next_prev))
}

// ============================================================================
// Main Parser Function
// ============================================================================

/// Parses a process snapshot on macOS using libproc.
///
/// Returns `(ProcessSnapshot, PrevProcessState)` where the second element is the
/// state to pass to the next invocation for delta calculations.
///
/// On the first sample (`prev == None`), cpu_percent returns
/// `MetricValue::Unavailable` since there's no baseline for deltas.
pub fn parse_process_snapshot(
    ctx: &SampleContext,
    prev: Option<&PrevProcessState>,
) -> Result<(ProcessSnapshot, PrevProcessState), TelemetryError> {
    let pids = get_pid_list()?;

    let mut next_prev = PrevProcessState::new();
    let mut processes = Vec::new();

    for pid in pids {
        let prev_counters = prev.and_then(|p| p.get(&(pid as u32)));
        if let Some((info, counters)) = parse_one_process(pid, ctx, prev_counters) {
            processes.push(info);
            next_prev.insert(pid as u32, counters);
        }
    }

    let snapshot = ProcessSnapshot {
        timestamp: ctx.now,
        total_count: processes.len() as u32,
        processes,
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

    fn fixed_ctx() -> SampleContext {
        SampleContext {
            now: chrono::Utc::now(),
            elapsed: Duration::from_secs(1),
            configured_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn process_snapshot_returns_processes() {
        let (snapshot, _prev) =
            parse_process_snapshot(&fixed_ctx(), None).expect("process snapshot should succeed");

        // Should have at least some processes (launchd, kernel_task, etc.)
        assert!(
            snapshot.total_count > 0,
            "should have at least one process"
        );
        assert!(
            !snapshot.processes.is_empty(),
            "processes vec should not be empty"
        );
    }

    #[test]
    fn first_sample_cpu_unavailable() {
        let (snapshot, _prev) =
            parse_process_snapshot(&fixed_ctx(), None).expect("process snapshot should succeed");

        // All processes should have cpu_percent unavailable on first sample
        for proc in &snapshot.processes {
            assert!(
                matches!(proc.cpu_percent, MetricValue::Unavailable { .. }),
                "cpu_percent should be unavailable on first sample for pid {}",
                proc.pid
            );
        }
    }

    #[test]
    fn second_sample_calculates_cpu() {
        let (_, prev) =
            parse_process_snapshot(&fixed_ctx(), None).expect("first sample should succeed");

        // Wait a bit
        std::thread::sleep(Duration::from_millis(100));

        let (snapshot, _) = parse_process_snapshot(&fixed_ctx(), Some(&prev))
            .expect("second sample should succeed");

        // At least some processes should have supported cpu_percent
        // (Not all may be running, so we check for at least one)
        let has_supported = snapshot
            .processes
            .iter()
            .any(|proc| matches!(proc.cpu_percent, MetricValue::Supported { .. }));

        assert!(
            has_supported,
            "at least one process should have supported cpu_percent on second sample"
        );
    }

    #[test]
    fn current_process_found() {
        let (snapshot, _) =
            parse_process_snapshot(&fixed_ctx(), None).expect("snapshot should succeed");

        let current_pid = std::process::id();
        let found = snapshot.processes.iter().any(|p| p.pid == current_pid);

        assert!(
            found,
            "current process (pid {}) should be in process list",
            current_pid
        );
    }

    #[test]
    fn rss_bytes_supported() {
        let (snapshot, _) =
            parse_process_snapshot(&fixed_ctx(), None).expect("snapshot should succeed");

        // All processes should have rss_bytes supported
        for proc in &snapshot.processes {
            assert!(
                matches!(proc.rss_bytes, MetricValue::Supported { .. }),
                "rss_bytes should be supported for pid {}",
                proc.pid
            );
        }
    }

    #[test]
    fn disk_and_network_io_unavailable() {
        let (snapshot, _) =
            parse_process_snapshot(&fixed_ctx(), None).expect("snapshot should succeed");

        // All processes should have disk/network I/O marked as unavailable
        for proc in &snapshot.processes {
            assert!(
                matches!(proc.disk_io_capability, Capability::Unavailable { .. }),
                "disk_io should be unavailable for pid {}",
                proc.pid
            );
            assert!(
                matches!(proc.network_io_capability, Capability::Unavailable { .. }),
                "network_io should be unavailable for pid {}",
                proc.pid
            );
        }
    }
}
