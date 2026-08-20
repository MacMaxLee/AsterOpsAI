//! `MetricSampler` is what `run_benchmark` polls to build the baseline and
//! post-change windows — decoupled from the SQLite telemetry pipeline
//! entirely (see docs/adr/0014: there's no live sampler running inside a
//! test, and a benchmark run needs its own tightly-scoped, directly-polled
//! metric anyway). `HostCpuUtilizationSampler` is the one real
//! implementation this unit ships, proving the concept end-to-end.

use crate::benchmark::error::BenchmarkError;

pub trait MetricSampler: Send + Sync {
    fn sample(&self) -> Result<f64, BenchmarkError>;
}

/// TRS §34 never says which way "better" points for a given metric — a
/// necessary, flagged addition: a verdict can't distinguish IMPROVED from
/// REGRESSED without knowing whether the benchmarked metric is one you
/// want lower (e.g. CPU utilization, latency) or higher (e.g. throughput).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricDirection {
    LowerIsBetter,
    HigherIsBetter,
}

#[cfg(target_os = "linux")]
mod linux_host_cpu {
    use std::sync::Mutex;
    use std::time::Duration;

    use super::MetricSampler;
    use crate::benchmark::error::BenchmarkError;

    /// Real host-aggregate CPU utilization, read directly from
    /// `/proc/stat` — deliberately a smaller, self-contained parse than
    /// `core::telemetry::cpu`'s own (which also tracks per-core detail,
    /// ctxt, and intr, none of which this sampler needs); not a
    /// duplication of that logic so much as a narrower one for a
    /// different purpose, the same class of judgment call as unit U8's
    /// own field-22 extraction note.
    pub struct HostCpuUtilizationSampler {
        prev: Mutex<Option<(u64, u64)>>,
    }

    impl HostCpuUtilizationSampler {
        pub fn new() -> Self {
            Self {
                prev: Mutex::new(None),
            }
        }

        fn read_aggregate_jiffies() -> Result<(u64, u64), BenchmarkError> {
            let raw = std::fs::read_to_string("/proc/stat")
                .map_err(|e| BenchmarkError::Sample(e.to_string()))?;
            let line = raw.lines().find(|l| l.starts_with("cpu ")).ok_or_else(|| {
                BenchmarkError::Sample("no aggregate cpu line in /proc/stat".to_string())
            })?;
            let fields: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|f| f.parse().ok())
                .collect();
            let idle = *fields
                .get(3)
                .ok_or_else(|| BenchmarkError::Sample("malformed /proc/stat".to_string()))?;
            // user+nice+system+idle+iowait+irq+softirq+steal, matching
            // core::telemetry::cpu's own total-jiffies convention.
            let total: u64 = fields.iter().take(8).sum();
            Ok((idle, total))
        }
    }

    impl Default for HostCpuUtilizationSampler {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MetricSampler for HostCpuUtilizationSampler {
        fn sample(&self) -> Result<f64, BenchmarkError> {
            let mut guard = self
                .prev
                .lock()
                .map_err(|_| BenchmarkError::Sample("poisoned lock".to_string()))?;
            let prev = match *guard {
                Some(p) => p,
                None => {
                    // Bootstrap: no previous snapshot to diff against on
                    // the very first call — take one, wait briefly (a
                    // real, short sleep), so even this first call returns
                    // a genuine rate rather than a fabricated placeholder.
                    let first = Self::read_aggregate_jiffies()?;
                    std::thread::sleep(Duration::from_millis(200));
                    first
                }
            };
            let current = Self::read_aggregate_jiffies()?;
            *guard = Some(current);

            let d_idle = current.0.saturating_sub(prev.0) as f64;
            let d_total = current.1.saturating_sub(prev.1) as f64;
            if d_total <= 0.0 {
                return Ok(0.0);
            }
            Ok(((d_total - d_idle) / d_total * 100.0).clamp(0.0, 100.0))
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux_host_cpu::HostCpuUtilizationSampler;
