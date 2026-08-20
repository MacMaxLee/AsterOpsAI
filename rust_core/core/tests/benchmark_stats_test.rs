//! Hand-computed cross-checks for `benchmark::stats` (TRS §34), beyond the
//! module's own inline unit tests — specifically exercising the
//! tie-correction term in the Mann-Whitney variance formula against a
//! fully hand-worked example. Integration tests are already test-only
//! code; the workspace's unwrap/expect deny targets production code
//! paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::benchmark::stats::{coefficient_of_variation, mann_whitney_u, median};

#[test]
fn coefficient_of_variation_hand_computed() {
    // mean = 5, sample variance (n-1) = ((1)^2+(-1)^2+(1)^2+(-1)^2)/3 = 4/3
    // stddev = sqrt(4/3) ≈ 1.1547, CoV = 1.1547/5 ≈ 0.23094
    let samples = [4.0, 6.0, 4.0, 6.0];
    let cv = coefficient_of_variation(&samples).expect("cv");
    assert!((cv - 0.230_940).abs() < 1e-4, "cv={cv}");
}

#[test]
fn median_hand_computed() {
    assert_eq!(median(&[7.0, 1.0, 4.0, 3.0, 9.0]), Some(4.0));
    assert_eq!(median(&[1.0, 2.0, 3.0, 100.0]), Some(2.5));
}

/// Fully hand-worked, including the tie-correction term:
/// a = [1, 2, 2, 3], b = [2, 3, 3, 4]. Combined ranks (average rank for
/// ties): 1->1, the three 2's (two from a, one from b) share ranks 2-4
/// (avg 3), the three 3's (one from a, two from b) share ranks 5-7 (avg 6),
/// 4->8. rank_sum_a = 1 + 3 + 3 + 6 = 13 (the lone "3" from a gets the
/// shared avg rank 6). U1 = 13 - 4*5/2 = 3; U2 = 16 - 3 = 13; U = min = 3.
/// tie_term = (3^3-3) + (3^3-3) = 24 + 24 = 48 (two tie groups of size 3).
/// variance = (16/12) * (9 - 48/(8*7)) ≈ 10.857 (hand-computed).
#[test]
fn mann_whitney_u_with_ties_matches_hand_computation() {
    let a = [1.0, 2.0, 2.0, 3.0];
    let b = [2.0, 3.0, 3.0, 4.0];
    let result = mann_whitney_u(&a, &b).expect("result");
    assert!(
        (result.u_statistic - 3.0).abs() < 1e-9,
        "u_statistic={}",
        result.u_statistic
    );
    // z ≈ -1.3657 by hand -> p ≈ 0.17; a generous band around the
    // hand-computed value, not an exact-to-the-last-digit assertion (the
    // erf approximation used for the normal CDF has its own, separately
    // documented ~1.5e-7 error bound).
    assert!(
        (0.10..0.25).contains(&result.p_value),
        "p_value={}",
        result.p_value
    );
}

#[test]
fn mann_whitney_no_ties_hand_computed_u_and_p_range() {
    // a = [1..5], b = [6..10]: completely separated, no ties.
    // U1 = 0 exactly (every a-value ranks below every b-value).
    // z ≈ -2.5067 by hand -> p ≈ 0.0122.
    let a = [1.0, 2.0, 3.0, 4.0, 5.0];
    let b = [6.0, 7.0, 8.0, 9.0, 10.0];
    let result = mann_whitney_u(&a, &b).expect("result");
    assert!((result.u_statistic - 0.0).abs() < 1e-9);
    assert!(
        (0.005..0.02).contains(&result.p_value),
        "p_value={}",
        result.p_value
    );
}
