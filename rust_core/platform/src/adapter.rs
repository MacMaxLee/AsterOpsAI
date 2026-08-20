use std::collections::BTreeSet;
use std::time::Duration;

use crate::error::CapabilityError;

/// This process's own resource usage, as reported by the OS.
#[derive(Debug, Clone, Copy)]
pub struct ProcessSelfMetrics {
    pub rss_bytes: u64,
    /// Cumulative user+system CPU time consumed by this process since it
    /// started. Callers derive a CPU% from the delta between two samples and
    /// a monotonic clock — never from a single reading.
    pub cpu_time: Duration,
}

/// A process scheduling priority (unit U8). Shaped after Windows' priority
/// *classes* (TRS §38 names `SetPriorityClass` as one of "the two U8
/// actions") rather than a raw numeric value, so the same enum has one
/// meaning across platforms even though only Linux has a real
/// implementation this unit. Deliberately excludes a Realtime tier — a
/// documented, flagged safety scope-down (see docs/adr/0013), not an
/// oversight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessPriority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
}

/// A process's allowed-CPU set. `cpus` holds logical CPU indices (matching
/// `CpuSnapshot.per_core_utilization_percent`'s own indexing, unit U1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuAffinityMask {
    pub cpus: BTreeSet<usize>,
}

/// One implementation per OS, selected at compile time. A method not
/// supported on a given platform returns `CapabilityError::Unsupported` — it
/// must compile and return this rather than being `#[cfg]`'d out, so the
/// capability model can describe it uniformly across platforms (TRS §5).
pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &'static str;

    fn arch(&self) -> &'static str {
        std::env::consts::ARCH
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError>;

    /// Unit U8. `pid` need not be a child of this process — any process
    /// this OS user has permission to inspect.
    fn get_process_priority(&self, pid: u32) -> Result<ProcessPriority, CapabilityError>;

    /// Raising priority (e.g. to `AboveNormal`/`High`) typically needs a
    /// privilege this process may not have — a real, expected
    /// `CapabilityError::PermissionRequired`, not something this method
    /// hides or works around.
    fn set_process_priority(
        &self,
        pid: u32,
        priority: ProcessPriority,
    ) -> Result<(), CapabilityError>;

    fn get_process_cpu_affinity(&self, pid: u32) -> Result<CpuAffinityMask, CapabilityError>;

    fn set_process_cpu_affinity(
        &self,
        pid: u32,
        mask: &CpuAffinityMask,
    ) -> Result<(), CapabilityError>;

    /// Unit U11's `SuspendProcess` security response action: stops the
    /// process's execution (`SIGSTOP` on Linux) without terminating it.
    fn suspend_process(&self, pid: u32) -> Result<(), CapabilityError>;

    /// Resumes a process stopped by `suspend_process` (`SIGCONT` on
    /// Linux) — `SuspendProcessAction`'s rollback.
    fn resume_process(&self, pid: u32) -> Result<(), CapabilityError>;

    /// `SuspendProcessAction`'s real verify step: is the process's live
    /// state actually stopped (`/proc/[pid]/stat`'s state field == `T` on
    /// Linux) — not merely "does `suspend_process` return `Ok`."
    fn is_process_stopped(&self, pid: u32) -> Result<bool, CapabilityError>;
}
