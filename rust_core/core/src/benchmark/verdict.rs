//! TRS §34's four post-hoc verdicts, plus the distinct pre-flight
//! `BaselineUnstable` abort (SRS FR-BENCH-001/002). `resolve` is pure: no
//! I/O, no AI, no dependency on `core::analysis`'s own scoring — a
//! benchmark verdict is its own, statistically-grounded concept.

use super::config::BenchmarkConfig;
use super::sampler::MetricDirection;
use super::stats::{coefficient_of_variation, mann_whitney_u, median};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkVerdict {
    Improved,
    Regressed,
    /// The honest default (SRS FR-BENCH-002) — no statistically
    /// significant difference was detected.
    Inconclusive,
    /// The *post-change* window itself failed the stability check — the
    /// data collected can't be trusted either way, distinct from
    /// `BaselineAbortReason::BaselineUnstable` (which never even applies
    /// the action).
    Unstable,
}

impl BenchmarkVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Improved => "IMPROVED",
            Self::Regressed => "REGRESSED",
            Self::Inconclusive => "INCONCLUSIVE",
            Self::Unstable => "UNSTABLE",
        }
    }
}

/// TRS §34's pre-flight abort: "if [the baseline's] coefficient of
/// variation exceeds a configured stability threshold, the run aborts
/// with `BASELINE_UNSTABLE` and no change is applied." A distinct return
/// path from `BenchmarkVerdict` — the action is never applied here.
pub const BASELINE_UNSTABLE: &str = "BASELINE_UNSTABLE";

/// Baseline/post-change window comparison. `direction` decides which way
/// "better" points for this metric (`sampler::MetricDirection` — TRS §34
/// doesn't name this, a necessary addition, see docs/adr/0014).
pub fn resolve(
    baseline: &[f64],
    post_change: &[f64],
    direction: MetricDirection,
    config: &BenchmarkConfig,
) -> BenchmarkVerdict {
    if let Some(cv) = coefficient_of_variation(post_change) {
        if cv >= config.stability_cv_threshold {
            return BenchmarkVerdict::Unstable;
        }
    }

    let Some(result) = mann_whitney_u(baseline, post_change) else {
        return BenchmarkVerdict::Inconclusive;
    };
    if result.p_value >= config.significance_level {
        return BenchmarkVerdict::Inconclusive;
    }

    let (Some(baseline_median), Some(post_change_median)) = (median(baseline), median(post_change))
    else {
        return BenchmarkVerdict::Inconclusive;
    };

    let improved = match direction {
        MetricDirection::LowerIsBetter => post_change_median < baseline_median,
        MetricDirection::HigherIsBetter => post_change_median > baseline_median,
    };
    if improved {
        BenchmarkVerdict::Improved
    } else {
        BenchmarkVerdict::Regressed
    }
}
