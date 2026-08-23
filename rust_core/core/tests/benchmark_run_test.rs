//! Real end-to-end coverage of `benchmark::run_benchmark`: a real spawned
//! process, real `host.*` actions executed through the real, unmodified
//! U7/U8 pipeline, and a deterministic scripted `MetricSampler` (see
//! `benchmark/common/mod.rs`'s own doc comment for why the *measured
//! values* are scripted while everything else is real). Linux-only, same
//! reasoning as `actions_host_priority_affinity_test.rs`.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "benchmark/common/mod.rs"]
mod bench_common;
#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use std::time::Duration;

use ai_ops_core::actions::host::{
    set_process_cpu_affinity_entry, set_process_priority_entry, ProcessTargetVerifier,
};
use ai_ops_core::benchmark::{
    run_benchmark, BenchmarkConfig, BenchmarkOutcome, BenchmarkPipeline, BenchmarkRunRequest,
    BenchmarkVerdict, MetricDirection,
};
use ai_ops_core::policy::{
    ActionRequest, ActionTypeRegistry, Environment, ProtectedResourceRegistry, ResourceDescriptor,
    ResourceKind,
};
use bench_common::{PhaseAwareScriptedSampler, ScriptedSampler};
use common::{test_action_context, SpawnedTarget};
use repo_common::TestRepo;

fn fast_config() -> BenchmarkConfig {
    BenchmarkConfig {
        min_window: Duration::from_millis(50),
        min_samples: 10,
        poll_interval: Duration::from_millis(5),
        stability_cv_threshold: 0.15,
        significance_level: 0.05,
    }
}

fn priority_registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register("host.set_process_priority", set_process_priority_entry());
    registry
}

fn affinity_registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register(
        "host.set_process_cpu_affinity",
        set_process_cpu_affinity_entry(),
    );
    registry
}

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

fn stable_around(value: f64, n: usize) -> Vec<f64> {
    (0..n).map(|i| value + [0.0, 0.1, -0.1][i % 3]).collect()
}

#[tokio::test]
async fn regressed_verdict_triggers_a_real_rollback() {
    // CPU affinity, not priority: docs/adr/0013 documents that restoring a
    // *lowered* nice value needs CAP_SYS_NICE this test's user doesn't
    // have, so a priority rollback can legitimately fail here — affinity
    // has no such asymmetry (U8's own test already proves a full
    // unprivileged round trip), letting this test assert a genuinely
    // successful real rollback rather than tolerating two possible
    // outcomes.
    let repo = TestRepo::open();
    let registry = affinity_registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let original_affinity = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read original affinity");

    // Baseline low & stable, post-change clearly higher: with
    // LowerIsBetter, that's an unambiguous regression. The phase switch is
    // keyed off the *real* live affinity (see PhaseAwareScriptedSampler's
    // own doc comment) rather than a guessed sample count, so there's no
    // risk of baseline-flavored values leaking into the post-change window.
    let target_pid = target.pid;
    let platform = context.platform.clone();
    let original_affinity_for_check = original_affinity.clone();
    let sampler = PhaseAwareScriptedSampler::new(
        move || {
            platform
                .get_process_cpu_affinity(target_pid)
                .map(|a| a != original_affinity_for_check)
                .unwrap_or(false)
        },
        stable_around(10.0, 60),
        stable_around(50.0, 60),
    );

    let request = BenchmarkRunRequest {
        action_request: ActionRequest {
            action_type: "host.set_process_cpu_affinity".to_string(),
            target: target.target(),
            resource: resource(),
            parameters: serde_json::json!({ "cpus": [0] }),
            requested_by: "tester".to_string(),
        },
        metric_name: "scripted_metric".to_string(),
        sampler: &sampler,
        direction: MetricDirection::LowerIsBetter,
        config: fast_config(),
    };
    let protected = ProtectedResourceRegistry::new();
    let pipeline = BenchmarkPipeline {
        registry: &registry,
        environment: Environment::Development,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        handle: &repo.handle,
    };

    let outcome = run_benchmark(request, &pipeline)
        .await
        .expect("run_benchmark");
    let (verdict, rolled_back) = match outcome {
        BenchmarkOutcome::Completed {
            verdict,
            rolled_back,
            ..
        } => (verdict, rolled_back),
        other => panic!("expected Completed, got {other:?}"),
    };
    assert_eq!(verdict, BenchmarkVerdict::Regressed);
    assert!(
        rolled_back,
        "a REGRESSED verdict must trigger a real, successful rollback"
    );

    let affinity_after = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity after rollback");
    assert_eq!(
        affinity_after, original_affinity,
        "a successful rollback must genuinely restore the original affinity"
    );

    target.ensure_reaped();
}

/// SRS FR-BENCH-001: the baseline window's own stability check aborts before applying anything.
#[tokio::test]
async fn baseline_unstable_never_applies_the_action() {
    let repo = TestRepo::open();
    let registry = priority_registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let original_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read original priority");

    // Wildly noisy baseline: coefficient of variation far above the 0.15
    // threshold.
    let mut script: Vec<f64> = (0..15)
        .map(|i| if i % 2 == 0 { 1.0 } else { 1000.0 })
        .collect();
    script.extend(stable_around(10.0, 15)); // never reached
    let sampler = ScriptedSampler::new(script);

    let request = BenchmarkRunRequest {
        action_request: ActionRequest {
            action_type: "host.set_process_priority".to_string(),
            target: target.target(),
            resource: resource(),
            parameters: serde_json::json!({ "priority": "BELOW_NORMAL" }),
            requested_by: "tester".to_string(),
        },
        metric_name: "scripted_metric".to_string(),
        sampler: &sampler,
        direction: MetricDirection::LowerIsBetter,
        config: fast_config(),
    };
    let protected = ProtectedResourceRegistry::new();
    let pipeline = BenchmarkPipeline {
        registry: &registry,
        environment: Environment::Development,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        handle: &repo.handle,
    };

    let outcome = run_benchmark(request, &pipeline)
        .await
        .expect("run_benchmark");
    match outcome {
        BenchmarkOutcome::BaselineAborted { .. } => {}
        other => panic!("expected BaselineAborted, got {other:?}"),
    }

    let priority_after = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after abort");
    assert_eq!(
        priority_after, original_priority,
        "a BaselineUnstable abort must never apply the action"
    );

    target.ensure_reaped();
}
