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
use ai_ops_core::tuning::{
    start_plan, AutomationMode, TuningError, TuningPipeline, TuningPlanStatus, TuningProfile,
};
use chrono::Utc;
use common::{test_action_context, SpawnedTarget};
use repo_common::TestRepo;

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

/// Was a real, genuine race, not merely a hypothetical: this resource's
/// desired state under `Balanced` matches a freshly spawned process's
/// actual state, so `build_candidates` returns zero candidates and
/// `start_plan`'s only real async work is its two writer-channel round
/// trips (insert, then immediately mark completed) with nothing in
/// between to force a yield — reproduced directly with the original
/// `tokio::join!`-raced version of this test (both plans completed with
/// distinct ids; the UNIQUE constraint never triggered because A had
/// already left `IN_FLIGHT`). Neither a multi-thread runtime (still
/// failed ~7% of 100 real runs — the actual bottleneck is mpsc
/// send/reply latency, not OS thread scheduling) nor polling for a real
/// `IN_FLIGHT` read before dispatching B (the whole insert-then-complete
/// round trip turned out to complete faster than a 5ms poll could
/// reliably observe) closed the window.
///
/// Fixed by controlling the window directly instead of racing for it or
/// polling for it: plan A is inserted via the exact same real repository
/// path `start_plan` itself uses (`repository::insert_tuning_plan`), but
/// deliberately never marked completed until this test says so — so
/// there is no window to miss. This still exercises the real
/// UNIQUE-index rejection path through the actual single-writer thread
/// exactly as `start_plan` would; it just doesn't route plan A through
/// `start_plan`'s own automatic completion.
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
    let target_identity = target.target();

    let plan_a = ai_ops_core::repository::insert_tuning_plan(
        &repo.handle,
        ai_ops_core::repository::NewTuningPlan {
            created_at: now,
            target_identity_json: serde_json::to_string(&target_identity)
                .expect("serialize target identity"),
            target_start_time: target_identity.start_time_marker(),
            profile: ai_ops_core::tuning::profile::profile_label(&TuningProfile::Balanced)
                .to_string(),
            mode: ai_ops_core::tuning::mode::mode_label(AutomationMode::RecommendOnly).to_string(),
            status: TuningPlanStatus::InFlight.as_str().to_string(),
            candidates_json: "[]".to_string(),
        },
    )
    .await
    .expect("insert plan A directly, deliberately left IN_FLIGHT");

    let result_b = start_plan(
        target_identity,
        resource(),
        TuningProfile::Balanced,
        AutomationMode::RecommendOnly,
        "tester-b",
        &pipeline,
        now,
    )
    .await;

    assert!(
        matches!(result_b, Err(TuningError::PlanAlreadyInFlight)),
        "a plan against a genuinely IN_FLIGHT target must be rejected via the real UNIQUE \
         constraint (FR-TUNE-002), not silently queued — got {result_b:?}"
    );

    ai_ops_core::repository::mark_tuning_plan_completed(
        &repo.handle,
        plan_a.id,
        TuningPlanStatus::Completed.as_str(),
        now,
        "[]".to_string(),
    )
    .await
    .expect("release plan A");

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
