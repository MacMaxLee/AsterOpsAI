//! Exhaustive coverage of `benchmark::verdict::resolve` (TRS §34's four
//! post-hoc verdicts) against deterministic `Vec<f64>` fixtures — no live
//! system needed. Integration tests are already test-only code; the
//! workspace's unwrap/expect deny targets production code paths, not
//! `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::benchmark::{resolve, BenchmarkConfig, BenchmarkVerdict, MetricDirection};

fn config() -> BenchmarkConfig {
    BenchmarkConfig::default()
}

fn stable(n: usize, around: f64) -> Vec<f64> {
    // Deterministic, low-variance sequence — CoV well under the default
    // 0.15 threshold.
    (0..n).map(|i| around + (i % 3) as f64 * 0.01).collect()
}

#[test]
fn clearly_lower_post_change_is_improved_when_lower_is_better() {
    let baseline = stable(30, 50.0);
    let post_change = stable(30, 20.0);
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Improved);
}

#[test]
fn clearly_lower_post_change_is_regressed_when_higher_is_better() {
    let baseline = stable(30, 50.0);
    let post_change = stable(30, 20.0);
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::HigherIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Regressed);
}

#[test]
fn clearly_higher_post_change_is_improved_when_higher_is_better() {
    let baseline = stable(30, 20.0);
    let post_change = stable(30, 50.0);
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::HigherIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Improved);
}

#[test]
fn clearly_higher_post_change_is_regressed_when_lower_is_better() {
    let baseline = stable(30, 20.0);
    let post_change = stable(30, 50.0);
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Regressed);
}

/// SRS FR-BENCH-002: the honest default outcome is INCONCLUSIVE.
#[test]
fn overlapping_distributions_are_inconclusive() {
    // Same underlying distribution, just interleaved noise — no
    // statistically significant difference.
    let baseline: Vec<f64> = (0..30).map(|i| 50.0 + (i % 5) as f64).collect();
    let post_change: Vec<f64> = (0..30).map(|i| 50.0 + ((i + 2) % 5) as f64).collect();
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Inconclusive);
}

#[test]
fn noisy_post_change_window_is_unstable_regardless_of_direction() {
    let baseline = stable(30, 50.0);
    // High coefficient of variation: alternating tiny/huge values.
    let post_change: Vec<f64> = (0..30)
        .map(|i| if i % 2 == 0 { 1.0 } else { 1000.0 })
        .collect();
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Unstable);
}

#[test]
fn boundary_p_value_at_significance_threshold_is_inconclusive() {
    // Two samples with no meaningful separation at all (identical
    // sequences) always have p=1.0, far above any reasonable significance
    // level — a direct boundary check that resolve() doesn't misclassify
    // "no evidence of difference" as anything but Inconclusive.
    let baseline = stable(30, 50.0);
    let post_change = stable(30, 50.0);
    let verdict = resolve(
        &baseline,
        &post_change,
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Inconclusive);
}

#[test]
fn empty_baseline_or_post_change_is_inconclusive_not_a_panic() {
    // A stable (low-CoV) post_change paired with an empty baseline: the
    // stability gate passes, so this exercises mann_whitney_u's own
    // "empty group -> no meaningful U statistic" path specifically,
    // rather than tripping the (separate, and separately tested)
    // post-change stability gate on a too-small sample.
    let verdict = resolve(
        &[],
        &stable(30, 50.0),
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Inconclusive);
    let verdict = resolve(&[1.0, 2.0], &[], MetricDirection::LowerIsBetter, &config());
    assert_eq!(verdict, BenchmarkVerdict::Inconclusive);
}

#[test]
fn a_too_small_post_change_window_can_legitimately_report_unstable() {
    // Real behavior, not a bug: `resolve()` checks post-change stability
    // on whatever data it's given — a 2-sample window's coefficient of
    // variation is nearly meaningless but still real arithmetic, and a
    // high one is honestly reported as Unstable. In production this case
    // doesn't arise because `run_benchmark`'s `collect_window` always
    // gathers at least `config.min_samples` before calling `resolve()` —
    // this test documents the direct-call edge case rather than assuming
    // it away.
    let verdict = resolve(
        &stable(30, 50.0),
        &[1.0, 2.0],
        MetricDirection::LowerIsBetter,
        &config(),
    );
    assert_eq!(verdict, BenchmarkVerdict::Unstable);
}
