pub mod exec;

use crate::{
    adapter::{CpuAffinityMask, ProcessPriority, ProcessSelfMetrics},
    error::CapabilityError,
    PlatformAdapter,
};

pub struct WindowsPlatformAdapter;

impl PlatformAdapter for WindowsPlatformAdapter {
    fn platform_name(&self) -> &'static str {
        "windows"
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows self-process metrics not implemented yet, see unit U12".to_string(),
        ))
    }

    // Real implementation is `SetPriorityClass`/`GetPriorityClass` (TRS
    // §38 names this exact API as one of "the two U8 actions") — not
    // implemented yet because Windows telemetry itself isn't (U12) and
    // this sandbox can't test it for real; stubbed the same way
    // `self_process_metrics` already is above.
    fn get_process_priority(&self, _pid: u32) -> Result<ProcessPriority, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows process priority not implemented yet, see unit U12".to_string(),
        ))
    }

    fn set_process_priority(
        &self,
        _pid: u32,
        _priority: ProcessPriority,
    ) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows process priority not implemented yet, see unit U12".to_string(),
        ))
    }

    // Real implementation is `SetProcessAffinityMask`/`GetProcessAffinityMask`
    // (TRS §38's other named U8 action) — same U12 deferral as above.
    fn get_process_cpu_affinity(&self, _pid: u32) -> Result<CpuAffinityMask, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows CPU affinity not implemented yet, see unit U12".to_string(),
        ))
    }

    fn set_process_cpu_affinity(
        &self,
        _pid: u32,
        _mask: &CpuAffinityMask,
    ) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows CPU affinity not implemented yet, see unit U12".to_string(),
        ))
    }

    // Real implementation is `SuspendThread`/`ResumeThread` per-thread (no
    // direct process-level SIGSTOP equivalent) — same U12 deferral as above.
    fn suspend_process(&self, _pid: u32) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows process suspend not implemented yet, see unit U12".to_string(),
        ))
    }

    fn resume_process(&self, _pid: u32) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows process resume not implemented yet, see unit U12".to_string(),
        ))
    }

    fn is_process_stopped(&self, _pid: u32) -> Result<bool, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows process state query not implemented yet, see unit U12".to_string(),
        ))
    }
}
