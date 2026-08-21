//! ADR 0034's own named gap, closed (unit U43/ADR 0048): "nothing
//! prevents the same target being suspended twice ... before either is
//! rolled back". Two real, genuinely concurrent `evaluate()` calls
//! against the same target and `action_type` — exactly one succeeds,
//! the other is rejected outright (not queued) via the real,
//! synchronous guard in `insert_proposed_action`, enforced through the
//! actual single-writer thread (ADR 0007), the same reasoning
//! `tuning_plan_concurrency_test.rs`'s own `UNIQUE`-index guard relies
//! on — reused here without a DB constraint, since this predicate is a
//! correlated subquery no SQLite partial index can express.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::policy::{
    evaluate, validate, ActionRequest, ActionTypeEntry, Environment, PolicyError, PolicyOutcome,
    ResourceDescriptor, ResourceKind, RiskLevel, TargetIdentity,
};
use chrono::Utc;
use common::lifecycle_test_registry;
use repo_common::TestRepo;

fn dummy_target() -> TargetIdentity {
    TargetIdentity::Process {
        pid: 424_242,
        start_time_ticks: 555_555,
    }
}

fn dummy_resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "dummy".to_string(),
    }
}

fn request(action_type: &str) -> ActionRequest {
    ActionRequest {
        action_type: action_type.to_string(),
        target: dummy_target(),
        resource: dummy_resource(),
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
    }
}

#[tokio::test]
async fn a_second_concurrent_proposal_for_the_same_target_and_action_type_is_rejected_not_queued() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();

    // Medium risk always requires approval (`PENDING_APPROVAL`) in any
    // environment — a real, unresolved status this guard must catch.
    let validated_a = validate(request("test.needs_approval"), &registry).expect("validate a");
    let validated_b = validate(request("test.needs_approval"), &registry).expect("validate b");

    let run_a = evaluate(
        validated_a,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    );
    let run_b = evaluate(
        validated_b,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    );

    let (result_a, result_b) = tokio::join!(run_a, run_b);
    let outcomes = [result_a, result_b];

    let success_count = outcomes.iter().filter(|r| r.is_ok()).count();
    let rejected_count = outcomes
        .iter()
        .filter(|r| matches!(r, Err(PolicyError::ConflictingActionInFlight)))
        .count();

    assert_eq!(
        success_count, 1,
        "exactly one concurrent proposal must succeed"
    );
    assert_eq!(
        rejected_count, 1,
        "the other must be rejected via the real guard, not silently queued"
    );
}

#[tokio::test]
async fn a_new_proposal_is_accepted_once_the_prior_one_is_denied() {
    let mut registry = lifecycle_test_registry();
    registry.register(
        "test.prohibited",
        ActionTypeEntry {
            risk_level: RiskLevel::Prohibited,
            reversible: false,
            validate_parameters: |_| Ok(()),
            construct: |_, _| unreachable!("a Prohibited action is never constructed"),
        },
    );
    let repo = TestRepo::open();

    let first_validated = validate(request("test.prohibited"), &registry).expect("validate");
    let first = evaluate(
        first_validated,
        Environment::Development,
        &registry,
        &repo.handle,
        Utc::now(),
    )
    .await
    .expect("first proposal");
    assert!(
        matches!(first, PolicyOutcome::Denied { .. }),
        "sanity: Prohibited risk always denies, got {first:?}"
    );

    let second_validated = validate(request("test.prohibited"), &registry).expect("validate");
    let second = evaluate(
        second_validated,
        Environment::Development,
        &registry,
        &repo.handle,
        Utc::now(),
    )
    .await;
    assert!(
        second.is_ok(),
        "a second proposal against the same target must be accepted once the first \
         resolved to a real terminal state (DENIED), got {second:?}"
    );
}
