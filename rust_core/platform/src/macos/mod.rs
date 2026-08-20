pub mod exec;

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
        Err(CapabilityError::Unsupported(
            "macos self-process metrics not implemented yet, see unit U12".to_string(),
        ))
    }

    // Real implementation is `setpriority(2)` (same POSIX call Linux uses)
    // — not implemented yet because macOS telemetry itself isn't (U12) and
    // this sandbox can't test it for real; stubbed the same way
    // `self_process_metrics` already is above.
    fn get_process_priority(&self, _pid: u32) -> Result<ProcessPriority, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos process priority not implemented yet, see unit U12".to_string(),
        ))
    }

    fn set_process_priority(
        &self,
        _pid: u32,
        _priority: ProcessPriority,
    ) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos process priority not implemented yet, see unit U12".to_string(),
        ))
    }

    // Real implementation is `thread_policy_set`/taskpolicy-style affinity
    // tagging (macOS has no direct `sched_setaffinity` equivalent) — same
    // U12 deferral as above.
    fn get_process_cpu_affinity(&self, _pid: u32) -> Result<CpuAffinityMask, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos CPU affinity not implemented yet, see unit U12".to_string(),
        ))
    }

    fn set_process_cpu_affinity(
        &self,
        _pid: u32,
        _mask: &CpuAffinityMask,
    ) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos CPU affinity not implemented yet, see unit U12".to_string(),
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
