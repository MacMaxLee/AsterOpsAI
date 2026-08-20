//! FR-TUNE-003's full chain, real end to end: a real prior benchmark run
//! producing a real `IMPROVED` verdict is what actually makes
//! `AutomationMode::AutoLowRisk` auto-execute a candidate — and a
//! same-shaped candidate for a *non-reversible* action type never does,
//! even with a matching improved-history row, proving the gate is really
//! three-part (`AutoAllowed` + `reversible` + improved history), not just
//! history alone. Linux-only, same reasoning as
//! `actions_host_priority_affinity_test.rs`.
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
use ai_ops_core::tuning::{start_plan, AutomationMode, TuningPipeline, TuningProfile};
use bench_common::PhaseAwareScriptedSampler;
use chrono::Utc;
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

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

fn stable_around(value: f64, n: usize) -> Vec<f64> {
    (0..n).map(|i| value + [0.0, 0.1, -0.1][i % 3]).collect()
}

/// Runs a real benchmark of `action_type` against `target` and asserts it
/// produced a real `IMPROVED` verdict — the improved-history fixture
/// `history::has_improved_history` will later find via the real
/// `benchmark_runs` JOIN `actions` query.
async fn seed_improved_history(
    repo: &TestRepo,
    registry: &ActionTypeRegistry,
    context: &ai_ops_core::actions::ActionContext,
    target: &SpawnedTarget,
    action_type: &str,
    parameters: serde_json::Value,
) {
    let target_pid = target.pid;
    let is_priority = action_type == "host.set_process_priority";
    let platform = context.platform.clone();
    let original_priority = context
        .platform
        .get_process_priority(target_pid)
        .expect("read original priority");
    let original_affinity = context
        .platform
        .get_process_cpu_affinity(target_pid)
        .expect("read original affinity");

    // Baseline high, post-change low: with LowerIsBetter, that's an
    // unambiguous improvement. The phase switch is keyed off the *real*
    // live state (see PhaseAwareScriptedSampler's own doc comment), not a
    // guessed sample count.
    let sampler = PhaseAwareScriptedSampler::new(
        move || {
            if is_priority {
                platform
                    .get_process_priority(target_pid)
                    .map(|p| p != original_priority)
                    .unwrap_or(false)
            } else {
                platform
                    .get_process_cpu_affinity(target_pid)
                    .map(|a| a != original_affinity)
                    .unwrap_or(false)
            }
        },
        stable_around(50.0, 60),
        stable_around(10.0, 60),
    );

    let request = BenchmarkRunRequest {
        action_request: ActionRequest {
            action_type: action_type.to_string(),
            target: target.target(),
            resource: resource(),
            parameters,
            requested_by: "tester".to_string(),
        },
        metric_name: "scripted_metric".to_string(),
        sampler: &sampler,
        direction: MetricDirection::LowerIsBetter,
        config: fast_config(),
    };
    let protected = ProtectedResourceRegistry::new();
    let pipeline = BenchmarkPipeline {
        registry,
        environment: Environment::Development,
        context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        handle: &repo.handle,
    };

    let outcome = run_benchmark(request, &pipeline)
        .await
        .expect("run_benchmark");
    match outcome {
        BenchmarkOutcome::Completed { verdict, .. } => {
            assert_eq!(
                verdict,
                BenchmarkVerdict::Improved,
                "fixture setup must genuinely produce IMPROVED"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[tokio::test]
async fn auto_low_risk_only_ever_auto_executes_the_reversible_candidate() {
    let repo = TestRepo::open();
    let context = test_action_context();

    let mut registry = ActionTypeRegistry::new();
    registry.register("host.set_process_priority", set_process_priority_entry());
    registry.register(
        "host.set_process_cpu_affinity",
        set_process_cpu_affinity_entry(),
    );

    // Real improved-history fixtures for BOTH action types — so the test
    // proves the gate excludes priority because it's not `reversible`,
    // never because history is merely missing.
    let history_target_priority = SpawnedTarget::spawn();
    seed_improved_history(
        &repo,
        &registry,
        &context,
        &history_target_priority,
        "host.set_process_priority",
        serde_json::json!({ "priority": "BELOW_NORMAL" }),
    )
    .await;
    history_target_priority.ensure_reaped();

    let history_target_affinity = SpawnedTarget::spawn();
    seed_improved_history(
        &repo,
        &registry,
        &context,
        &history_target_affinity,
        "host.set_process_cpu_affinity",
        serde_json::json!({ "cpus": [0] }),
    )
    .await;
    history_target_affinity.ensure_reaped();

    // The real target the AutoLowRisk plan runs against — untouched by
    // either fixture above.
    let target = SpawnedTarget::spawn();
    let original_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read original priority");
    let original_affinity = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read original affinity");
    assert_ne!(
        original_priority,
        platform::ProcessPriority::BelowNormal,
        "test assumes the freshly spawned target does not already run BELOW_NORMAL"
    );

    let protected = ProtectedResourceRegistry::new();
    let pipeline = TuningPipeline {
        handle: &repo.handle,
        registry: &registry,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        environment: Environment::Development,
    };

    // BatterySaver wants BelowNormal + {cpu 0} — differs from a freshly
    // spawned process's default Normal + full-mask state on both fields,
    // so both actions are proposed.
    let outcome = start_plan(
        target.target(),
        resource(),
        TuningProfile::BatterySaver,
        AutomationMode::AutoLowRisk,
        "tester",
        &pipeline,
        Utc::now(),
    )
    .await
    .expect("start_plan");

    let priority_outcome = outcome
        .candidates
        .iter()
        .find(|c| c.action_type == "host.set_process_priority")
        .expect("a priority candidate must have been proposed");
    let affinity_outcome = outcome
        .candidates
        .iter()
        .find(|c| c.action_type == "host.set_process_cpu_affinity")
        .expect("an affinity candidate must have been proposed");

    assert_eq!(
        affinity_outcome.outcome, "AUTO_EXECUTED",
        "reversible + AutoAllowed + improved history must auto-execute"
    );
    assert_ne!(
        priority_outcome.outcome, "AUTO_EXECUTED",
        "a non-reversible action type must never auto-execute, even with matching history"
    );

    let live_affinity = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity after the plan");
    assert_ne!(
        live_affinity, original_affinity,
        "the auto-executed affinity change must genuinely have happened"
    );

    let live_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after the plan");
    assert_eq!(
        live_priority, original_priority,
        "the non-auto-executed priority candidate must never have touched live state"
    );

    target.ensure_reaped();
}
