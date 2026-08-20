//! TRS §34's own numbers, plus two judgment calls it leaves unspecified
//! (documented in docs/adr/0014, same precedent as ADR 0006/0010's own
//! threshold tables). `::default()` matches TRS exactly; every field is
//! overridable so tests can use a scaled-down window without touching
//! production behavior.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct BenchmarkConfig {
    /// TRS §34: "a window of at least 60 seconds and at least 30 samples."
    pub min_window: Duration,
    pub min_samples: usize,
    /// How often the orchestrator polls the `MetricSampler` while filling
    /// a window.
    pub poll_interval: Duration,
    /// A documented judgment call (TRS §34 requires *a* stability
    /// threshold, not a specific number): a coefficient of variation at or
    /// above this is "too noisy to trust," on both the baseline (pre-flight
    /// `BaselineUnstable` abort) and the post-change window (post-hoc
    /// `Unstable` verdict).
    pub stability_cv_threshold: f64,
    /// The standard two-tailed significance level for the Mann-Whitney U
    /// test's p-value — a documented judgment call, not pinned by TRS.
    pub significance_level: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            min_window: Duration::from_secs(60),
            min_samples: 30,
            poll_interval: Duration::from_secs(2),
            stability_cv_threshold: 0.15,
            significance_level: 0.05,
        }
    }
}
