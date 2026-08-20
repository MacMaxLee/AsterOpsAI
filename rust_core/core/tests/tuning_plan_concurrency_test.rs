//! FR-TUNE-002: at most one tuning plan may be in flight per target. Two
//! real, genuinely concurrent `start_plan` calls against the same real
//! spawned target — exactly one succeeds, the other is rejected outright
//! (not queued) via the real partial `UNIQUE` index
//! (`idx_tuning_plans_one_in_flight_per_target`, migrations/V11), enforced
//! through the actual single-writer thread (ADR 0007), not an in-memory
//! lock. Linux-only, same reasoning as `actions_host_priority_affinity_test.rs`.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::actions::host::ProcessTargetVerifier;
use ai_ops_core::policy::{
    ActionTypeRegistry, Environment, ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use ai_ops_core::tuning::{start_plan, AutomationMode, TuningError, TuningPipeline, TuningProfile};
use chrono::Utc;
use common::{test_action_context, SpawnedTarget};
use repo_common::TestRepo;

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

#[tokio::test]
async fn a_second_concurrent_plan_for_the_same_target_is_rejected_not_queued() {
    let repo = TestRepo::open();
    let registry = ActionTypeRegistry::new();
    let context = test_action_context();
    let protected = ProtectedResourceRegistry::new();
    let target = SpawnedTarget::spawn();

    let pipeline = TuningPipeline {
        handle: &repo.handle,
        registry: &registry,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        environment: Environment::Development,
    };

    let now = Utc::now();
    let run_a = start_plan(
        target.target(),
        resource(),
        TuningProfile::Balanced,
        AutomationMode::RecommendOnly,
        "tester-a",
        &pipeline,
        now,
    );
    let run_b = start_plan(
        target.target(),
        resource(),
        TuningProfile::Balanced,
        AutomationMode::RecommendOnly,
        "tester-b",
        &pipeline,
        now,
    );

    let (result_a, result_b) = tokio::join!(run_a, run_b);
    let outcomes = [result_a, result_b];

    let success_count = outcomes.iter().filter(|r| r.is_ok()).count();
    let rejected_count = outcomes
        .iter()
        .filter(|r| matches!(r, Err(TuningError::PlanAlreadyInFlight)))
        .count();

    assert_eq!(success_count, 1, "exactly one concurrent plan must succeed");
    assert_eq!(
        rejected_count, 1,
        "the other must be rejected via the real UNIQUE constraint (FR-TUNE-002), not silently queued"
    );

    target.ensure_reaped();
}

#[tokio::test]
async fn a_new_plan_is_accepted_once_the_prior_one_has_completed() {
    let repo = TestRepo::open();
    let registry = ActionTypeRegistry::new();
    let context = test_action_context();
    let protected = ProtectedResourceRegistry::new();
    let target = SpawnedTarget::spawn();

    let pipeline = TuningPipeline {
        handle: &repo.handle,
        registry: &registry,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        environment: Environment::Development,
    };

    let first = start_plan(
        target.target(),
        resource(),
        TuningProfile::Balanced,
        AutomationMode::RecommendOnly,
        "tester",
        &pipeline,
        Utc::now(),
    )
    .await
    .expect("first plan");
    assert!(first.plan_id > 0, "sanity: a real row id was assigned");

    let second = start_plan(
        target.target(),
        resource(),
        TuningProfile::Balanced,
        AutomationMode::RecommendOnly,
        "tester",
        &pipeline,
        Utc::now(),
    )
    .await;
    assert!(
        second.is_ok(),
        "a second plan against the same target must be accepted once the first has \
         transitioned out of IN_FLIGHT, got {second:?}"
    );

    target.ensure_reaped();
}
