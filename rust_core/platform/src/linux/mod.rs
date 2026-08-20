pub mod exec;
pub mod proc_source;
pub mod process_control;
pub mod storage;

use std::time::Duration;

use crate::{
    adapter::{CpuAffinityMask, ProcessPriority, ProcessSelfMetrics},
    error::CapabilityError,
    PlatformAdapter,
};

pub use proc_source::{ProcSource, RealProcSource};
pub use storage::{volume_capacity, VolumeCapacity};

pub struct LinuxPlatformAdapter;

impl PlatformAdapter for LinuxPlatformAdapter {
    fn platform_name(&self) -> &'static str {
        "linux"
    }

    fn get_process_priority(&self, pid: u32) -> Result<ProcessPriority, CapabilityError> {
        process_control::get_priority(pid)
    }

    fn set_process_priority(
        &self,
        pid: u32,
        priority: ProcessPriority,
    ) -> Result<(), CapabilityError> {
        process_control::set_priority(pid, priority)
    }

    fn get_process_cpu_affinity(&self, pid: u32) -> Result<CpuAffinityMask, CapabilityError> {
        process_control::get_cpu_affinity(pid)
    }

    fn set_process_cpu_affinity(
        &self,
        pid: u32,
        mask: &CpuAffinityMask,
    ) -> Result<(), CapabilityError> {
        process_control::set_cpu_affinity(pid, mask)
    }

    fn suspend_process(&self, pid: u32) -> Result<(), CapabilityError> {
        process_control::suspend(pid)
    }

    fn resume_process(&self, pid: u32) -> Result<(), CapabilityError> {
        process_control::resume(pid)
    }

    fn is_process_stopped(&self, pid: u32) -> Result<bool, CapabilityError> {
        process_control::is_stopped(pid)
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError> {
        let usage = get_rusage_self()?;

        let cpu_time =
            Duration::from_secs(usage.ru_utime.tv_sec as u64 + usage.ru_stime.tv_sec as u64)
                + Duration::from_micros(
                    usage.ru_utime.tv_usec as u64 + usage.ru_stime.tv_usec as u64,
                );
        // Linux reports ru_maxrss in kilobytes.
        let rss_bytes = usage.ru_maxrss as u64 * 1024;

        Ok(ProcessSelfMetrics {
            rss_bytes,
            cpu_time,
        })
    }
}

/// Narrow, this-process-only syscall wrapper — reads the kernel's own
/// accounting for this process, not `/proc`. Distinct from the
/// fixture-testable `ProcSource` abstraction reserved for host telemetry in
/// unit U1 (TRS §6); this exists only to answer "how much CPU/RSS is this
/// service itself using" for `/health`.
#[allow(unsafe_code)]
fn get_rusage_self() -> Result<libc::rusage, CapabilityError> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a valid, correctly-sized, writable buffer for the
    // duration of this call; `getrusage` only ever writes to it and returns
    // a status code we check before treating it as initialized.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return Err(CapabilityError::Io(std::io::Error::last_os_error()));
    }
    // SAFETY: `rc == 0` guarantees the kernel fully populated `usage`.
    Ok(unsafe { usage.assume_init() })
}
