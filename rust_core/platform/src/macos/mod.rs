pub mod exec;
pub mod process_control;

use std::time::Duration;

use crate::{
    adapter::{CpuAffinityMask, ProcessPriority, ProcessSelfMetrics},
    error::CapabilityError,
    PlatformAdapter,
};

pub struct MacosPlatformAdapter;

impl PlatformAdapter for MacosPlatformAdapter {
    fn platform_name(&self) -> &'static str {
        "macos"
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError> {
        let usage = get_rusage_self()?;

        let cpu_time =
            Duration::from_secs(usage.ru_utime.tv_sec as u64 + usage.ru_stime.tv_sec as u64)
                + Duration::from_micros(
                    usage.ru_utime.tv_usec as u64 + usage.ru_stime.tv_usec as u64,
                );
        // macOS reports ru_maxrss in bytes (Linux reports in kilobytes).
        let rss_bytes = usage.ru_maxrss as u64;

        Ok(ProcessSelfMetrics {
            rss_bytes,
            cpu_time,
        })
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

    fn get_process_cpu_affinity(&self, _pid: u32) -> Result<CpuAffinityMask, CapabilityError> {
        // macOS has no sched_setaffinity equivalent — thread_policy_set is
        // per-thread and advisory only, not a process-level hard mask.
        // See ADR 0086 for full rationale.
        Err(CapabilityError::Unsupported(
            "macOS lacks process-level hard CPU affinity masks; \
             see ADR 0086 for rationale (no sched_setaffinity equivalent)"
                .to_string(),
        ))
    }

    fn set_process_cpu_affinity(
        &self,
        _pid: u32,
        _mask: &CpuAffinityMask,
    ) -> Result<(), CapabilityError> {
        // macOS has no sched_setaffinity equivalent — thread_policy_set is
        // per-thread and advisory only, not a process-level hard mask.
        // See ADR 0086 for full rationale.
        Err(CapabilityError::Unsupported(
            "macOS lacks process-level hard CPU affinity masks; \
             see ADR 0086 for rationale (no sched_setaffinity equivalent)"
                .to_string(),
        ))
    }

    // Real implementation is `kill(2)` with SIGSTOP/SIGCONT, same POSIX
    // call Linux uses — same U12 deferral as above.
    fn suspend_process(&self, _pid: u32) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos process suspend not implemented yet, see unit U12".to_string(),
        ))
    }

    fn resume_process(&self, _pid: u32) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos process resume not implemented yet, see unit U12".to_string(),
        ))
    }

    fn is_process_stopped(&self, _pid: u32) -> Result<bool, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos process state query not implemented yet, see unit U12".to_string(),
        ))
    }
}

/// Narrow, this-process-only syscall wrapper — reads the kernel's own
/// accounting for this process. Distinct from the host telemetry layer;
/// this exists only to answer "how much CPU/RSS is this service itself
/// using" for `/health`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_process_metrics_returns_real_values() {
        let adapter = MacosPlatformAdapter;
        let metrics = adapter
            .self_process_metrics()
            .expect("self_process_metrics should succeed");

        // RSS should be greater than zero (this process is running and using memory)
        assert!(
            metrics.rss_bytes > 0,
            "RSS should be > 0, got {}",
            metrics.rss_bytes
        );

        // CPU time should be greater than zero (this process has consumed some CPU)
        assert!(
            metrics.cpu_time.as_nanos() > 0,
            "CPU time should be > 0, got {:?}",
            metrics.cpu_time
        );
    }
}
