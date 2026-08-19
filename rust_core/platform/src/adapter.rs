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
}
