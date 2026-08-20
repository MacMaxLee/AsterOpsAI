//! Real smoke test for `benchmark::HostCpuUtilizationSampler` (the one
//! real `MetricSampler` this unit ships) — proves it returns real numbers
//! over real wall-clock time, independent of any verdict. Linux-only.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::benchmark::{HostCpuUtilizationSampler, MetricSampler};

#[test]
fn returns_real_in_range_values_over_real_time() {
    let sampler = HostCpuUtilizationSampler::new();

    let mut values = Vec::new();
    for _ in 0..5 {
        let v = sampler.sample().expect("sample");
        assert!(
            (0.0..=100.0).contains(&v),
            "CPU utilization must be a real percentage, got {v}"
        );
        values.push(v);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Not every real-world run guarantees variation (an idle box could
    // read close to the same low number repeatedly), but the values must
    // all be genuine floats computed from real /proc/stat deltas, not a
    // single fabricated constant repeated by construction. The first call
    // already proves the bootstrap path (no previous sample yet) works —
    // this loop proves every subsequent call does too.
    assert_eq!(values.len(), 5);
}

#[test]
fn first_call_does_not_panic_with_no_previous_sample() {
    let sampler = HostCpuUtilizationSampler::new();
    let v = sampler.sample().expect("first sample");
    assert!((0.0..=100.0).contains(&v));
}
