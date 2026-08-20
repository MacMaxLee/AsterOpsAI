//! Hand-rolled, dependency-free statistics (TRS §34): no RNG, no external
//! crate — see docs/adr/0014 for why Mann-Whitney U was chosen over a
//! bootstrap confidence interval (TRS §34 permits either). Every function
//! here is pure arithmetic over a `&[f64]`, no I/O.

/// Sample standard deviation / mean (`n-1` denominator, the usual
/// estimator for a sample rather than a full population). `None` when
/// there are fewer than 2 samples, or the mean is exactly zero (a CoV is
/// undefined there, not infinite-and-therefore-unstable).
pub fn coefficient_of_variation(samples: &[f64]) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<f64>() / n;
    if mean == 0.0 {
        return None;
    }
    let variance = samples.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    Some(variance.sqrt() / mean.abs())
}

pub fn median(samples: &[f64]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mid = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    })
}

#[derive(Debug, Clone, Copy)]
pub struct MannWhitneyResult {
    pub u_statistic: f64,
    /// Two-tailed p-value from the normal approximation with a tie
    /// correction and continuity correction — accurate for the sample
    /// sizes TRS §34 already requires (>=30 per group).
    pub p_value: f64,
}

/// Abramowitz & Stegun 7.1.26 — a standard, ~1.5e-7-accurate error
/// function approximation. The only reason this exists is to compute a
/// normal CDF for the Mann-Whitney p-value without a new dependency.
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    const P: f64 = 0.3275911;
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - (((((A5 * t + A4) * t) + A3) * t + A2) * t + A1) * t * (-x * x).exp();
    sign * y
}

fn standard_normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Average-rank assignment over the combined, sorted sample — ties share
/// the mean of the ranks they'd otherwise occupy, the standard Mann-Whitney
/// tie-handling rule. Returns `(rank, is_from_a)` pairs in the same sorted
/// order, plus the tie-correction term `Σ(t_i^3 - t_i)` used by the
/// variance formula below.
fn ranks_with_ties(a: &[f64], b: &[f64]) -> (Vec<(f64, bool)>, f64) {
    let mut combined: Vec<(f64, bool)> = a
        .iter()
        .map(|&v| (v, true))
        .chain(b.iter().map(|&v| (v, false)))
        .collect();
    combined.sort_by(|x, y| x.0.total_cmp(&y.0));

    let mut ranked = Vec::with_capacity(combined.len());
    let mut tie_term = 0.0;
    let mut i = 0;
    while i < combined.len() {
        let mut j = i + 1;
        while j < combined.len() && combined[j].0 == combined[i].0 {
            j += 1;
        }
        // Positions i..j (0-indexed) occupy ranks i+1..=j (1-indexed);
        // their average rank is the mean of that inclusive range.
        let tie_count = (j - i) as f64;
        let avg_rank = ((i + 1) as f64 + j as f64) / 2.0;
        for &(_, is_a) in &combined[i..j] {
            ranked.push((avg_rank, is_a));
        }
        if tie_count > 1.0 {
            tie_term += tie_count.powi(3) - tie_count;
        }
        i = j;
    }
    (ranked, tie_term)
}

/// `a`/`b` need at least one sample each (returns `None` otherwise — no
/// meaningful U statistic exists with an empty group).
pub fn mann_whitney_u(a: &[f64], b: &[f64]) -> Option<MannWhitneyResult> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let n1 = a.len() as f64;
    let n2 = b.len() as f64;
    let n = n1 + n2;

    let (ranked, tie_term) = ranks_with_ties(a, b);
    let rank_sum_a: f64 = ranked
        .iter()
        .filter(|(_, is_a)| *is_a)
        .map(|(r, _)| r)
        .sum();

    let u1 = rank_sum_a - n1 * (n1 + 1.0) / 2.0;
    let u2 = n1 * n2 - u1;
    let u = u1.min(u2);

    let mean_u = n1 * n2 / 2.0;
    let variance = if n > 1.0 {
        (n1 * n2 / 12.0) * ((n + 1.0) - tie_term / (n * (n - 1.0)))
    } else {
        0.0
    };
    if variance <= 0.0 {
        // Every value tied across both groups — no distinguishing signal.
        return Some(MannWhitneyResult {
            u_statistic: u,
            p_value: 1.0,
        });
    }
    let sigma = variance.sqrt();
    let diff = u1 - mean_u;
    let continuity = 0.5 * diff.signum();
    let z = (diff - continuity) / sigma;
    let p_value = 2.0 * (1.0 - standard_normal_cdf(z.abs()));

    Some(MannWhitneyResult {
        u_statistic: u,
        p_value: p_value.clamp(0.0, 1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_of_variation_of_constant_samples_is_zero() {
        let cv = coefficient_of_variation(&[10.0, 10.0, 10.0]).expect("cv");
        assert!(cv.abs() < 1e-9);
    }

    #[test]
    fn coefficient_of_variation_needs_at_least_two_samples() {
        assert!(coefficient_of_variation(&[5.0]).is_none());
        assert!(coefficient_of_variation(&[]).is_none());
    }

    #[test]
    fn coefficient_of_variation_zero_mean_is_none() {
        assert!(coefficient_of_variation(&[-1.0, 1.0]).is_none());
    }

    #[test]
    fn median_of_odd_and_even_length() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn mann_whitney_identical_distributions_is_high_p_value() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let result = mann_whitney_u(&a, &b).expect("result");
        assert!(result.p_value > 0.9, "p={}", result.p_value);
    }

    #[test]
    fn mann_whitney_clearly_separated_distributions_is_significant() {
        let a: Vec<f64> = (0..30).map(|i| i as f64).collect(); // 0..29
        let b: Vec<f64> = (0..30).map(|i| i as f64 + 100.0).collect(); // 100..129
        let result = mann_whitney_u(&a, &b).expect("result");
        assert!(result.p_value < 0.001, "p={}", result.p_value);
    }

    /// A textbook-style hand-checkable case: two small, non-overlapping
    /// groups where U is exactly computable by hand (U=0 for the "a"
    /// group, since every value in b exceeds every value in a).
    #[test]
    fn mann_whitney_hand_computed_u_statistic() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let result = mann_whitney_u(&a, &b).expect("result");
        // rank_sum_a = 1+2+3 = 6; U1 = 6 - 3*4/2 = 0; U2 = 9-0 = 9; U=min=0.
        assert!((result.u_statistic - 0.0).abs() < 1e-9);
    }

    #[test]
    fn mann_whitney_requires_non_empty_groups() {
        assert!(mann_whitney_u(&[], &[1.0]).is_none());
        assert!(mann_whitney_u(&[1.0], &[]).is_none());
    }
}
