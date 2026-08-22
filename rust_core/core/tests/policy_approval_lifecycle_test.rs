//! TRS §25's approval lifecycle (SRS FR-POL-003: explicit, single-use,
//! time-limited approval bound to a target/parameter set): single-use,
//! expiry, replay, parameter mutation, and target change are each a
//! dedicated, real test against the real SQLite writer thread — not an
//! in-memory stand-in. Integration
//! tests are already test-only code; the workspace's unwrap/expect deny
//! targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, Environment, PolicyOutcome, ResourceDescriptor,
    ResourceKind, TargetIdentity,
};
use ai_ops_core::repository;
use chrono::{Duration, Utc};
use common::lifecycle_test_registry;
use repo_common::TestRepo;

fn dummy_target() -> TargetIdentity {
    TargetIdentity::Process {
        pid: 999_999,
        start_time_ticks: 123_456,
    }
}

fn dummy_resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "dummy".to_string(),
    }
}

fn needs_approval_request() -> ActionRequest {
    ActionRequest {
        action_type: "test.needs_approval".to_string(),
        target: dummy_target(),
        resource: dummy_resource(),
        parameters: serde_json::json!({"n": 1}),
        requested_by: "tester".to_string(),
    }
}

async fn propose_and_grant(repo: &repo_common::TestRepo, now: chrono::DateTime<Utc>) -> i64 {
    let registry = lifecycle_test_registry();
    let validated = validate(needs_approval_request(), &registry).expect("validate");
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::PendingApproval { row_id, .. } => row_id,
        other => panic!("expected PendingApproval, got {other:?}"),
    };
    approval::grant(&repo.handle, row_id, "approver", now)
        .await
        .expect("grant");
    row_id
}

#[tokio::test]
async fn auto_allowed_actions_skip_grant_and_authorize_directly() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let request = ActionRequest {
        action_type: "test.auto_allow".to_string(),
        ..needs_approval_request()
    };
    let validated = validate(request, &registry).expect("validate");
    let now = Utc::now();
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::AutoAllowed { row_id } => row_id,
        other => panic!("expected AutoAllowed, got {other:?}"),
    };

    let approved = approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &serde_json::json!({"n": 1}),
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        now,
    )
    .await
    .expect("authorize should succeed without ever calling grant()");
    assert_eq!(approved.row_id(), row_id);
}

#[tokio::test]
async fn single_use_a_second_authorize_call_on_the_same_row_fails() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let row_id = propose_and_grant(&repo, now).await;

    let params = serde_json::json!({"n": 1});
    approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &params,
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        now,
    )
    .await
    .expect("first authorize should succeed");

    let second = approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &params,
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        now,
    )
    .await;
    assert!(
        second.is_err(),
        "a second authorize() on an already-consumed row must fail"
    );
}

#[tokio::test]
async fn replay_of_a_consumed_approval_after_a_delay_still_fails() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let t0 = Utc::now();
    let row_id = propose_and_grant(&repo, t0).await;

    let params = serde_json::json!({"n": 1});
    approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &params,
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        t0,
    )
    .await
    .expect("first authorize should succeed");

    // Simulate a replay arriving well after the first consumption.
    let replay_time = t0 + Duration::seconds(60);
    let replay = approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &params,
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        replay_time,
    )
    .await;
    assert!(replay.is_err(), "a replayed approval must be rejected");
}

#[tokio::test]
async fn expired_approval_is_rejected() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let t0 = Utc::now();
    let row_id = propose_and_grant(&repo, t0).await;

    // The default TTL is 300s (DEFAULT_APPROVAL_TTL_SECONDS) — simulate
    // `now` well past that at authorize time, matching this project's
    // established "caller-supplied simulated timestamps" pattern for
    // time-based logic (see repository::run_retention_sweep's own doc
    // comment) rather than a real 5-minute wait.
    let past_expiry = t0 + Duration::seconds(ai_ops_core::policy::DEFAULT_APPROVAL_TTL_SECONDS + 1);
    let result = approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &serde_json::json!({"n": 1}),
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        past_expiry,
    )
    .await;
    assert!(result.is_err(), "an expired approval must be rejected");
}

#[tokio::test]
async fn grant_itself_rejects_an_already_expired_pending_approval() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let t0 = Utc::now();
    let validated = validate(needs_approval_request(), &registry).expect("validate");
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        t0,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::PendingApproval { row_id, .. } => row_id,
        other => panic!("expected PendingApproval, got {other:?}"),
    };

    let past_expiry = t0 + Duration::seconds(ai_ops_core::policy::DEFAULT_APPROVAL_TTL_SECONDS + 1);
    let result = approval::grant(&repo.handle, row_id, "approver", past_expiry).await;
    assert!(
        result.is_err(),
        "granting an already-expired proposal must be rejected"
    );
}

#[tokio::test]
async fn mutated_parameters_after_approval_are_rejected() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let row_id = propose_and_grant(&repo, now).await;

    let mutated_params = serde_json::json!({"n": 2}); // proposed with n=1
    let result = approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &mutated_params,
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        now,
    )
    .await;
    assert!(
        result.is_err(),
        "authorize() with different parameters than what was proposed must fail"
    );
}

#[tokio::test]
async fn changed_target_after_approval_is_rejected() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let row_id = propose_and_grant(&repo, now).await;

    let different_target = TargetIdentity::Process {
        pid: 999_999,
        start_time_ticks: 999_999, // different start time -> different identity
    };
    let result = approval::authorize(
        &repo.handle,
        row_id,
        &different_target,
        &serde_json::json!({"n": 1}),
        &dummy_resource(),
        &registry,
        &common::test_action_context(),
        now,
    )
    .await;
    assert!(
        result.is_err(),
        "authorize() with a different target than what was proposed must fail"
    );
}

#[tokio::test]
async fn changed_resource_after_approval_is_rejected() {
    // Regression test: authorize() originally checked target/parameters
    // deviation but not resource — a caller could propose against one
    // resource and authorize against a relabeled one, which
    // `actions::execute`'s protected-resource stage would then trust
    // instead of what was actually approved (found by adversarial review
    // after U7 shipped, see docs/adr/0012's follow-up note).
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let row_id = propose_and_grant(&repo, now).await;

    let different_resource = ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "a-different-resource-name".to_string(),
    };
    let result = approval::authorize(
        &repo.handle,
        row_id,
        &dummy_target(),
        &serde_json::json!({"n": 1}),
        &different_resource,
        &registry,
        &common::test_action_context(),
        now,
    )
    .await;
    assert!(
        result.is_err(),
        "authorize() with a different resource than what was proposed must fail"
    );
}

/// Unit U54: no core-level `reject` coverage existed in this file before
/// this unit — baseline proof of the pre-existing `PENDING_APPROVAL`
/// path, unmodified by this unit's `AUTO_ALLOWED` addition below.
#[tokio::test]
async fn reject_a_pending_approval_action_denies_it_and_audits_as_rejected() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let validated = validate(needs_approval_request(), &registry).expect("validate");
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::PendingApproval { row_id, .. } => row_id,
        other => panic!("expected PendingApproval, got {other:?}"),
    };

    approval::reject(&repo.handle, row_id, "reviewer", now)
        .await
        .expect("reject");

    let row = repository::get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "DENIED");

    let event_type = repository::latest_audit_event_type(&repo.handle)
        .await
        .expect("latest_audit_event_type")
        .expect("an audit event exists");
    assert_eq!(event_type, "policy.rejected");
}

/// Unit U54 (ADR 0055/0059): the new capability — a real `AUTO_ALLOWED`
/// row (never itself calling `grant`) can now be vetoed before it ever
/// runs, denied for real, and audited under its own distinct event type.
#[tokio::test]
async fn reject_an_auto_allowed_action_denies_it_and_audits_distinctly() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let request = ActionRequest {
        action_type: "test.auto_allow".to_string(),
        ..needs_approval_request()
    };
    let validated = validate(request, &registry).expect("validate");
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::AutoAllowed { row_id } => row_id,
        other => panic!("expected AutoAllowed, got {other:?}"),
    };

    approval::reject(&repo.handle, row_id, "reviewer", now)
        .await
        .expect("reject an auto-allowed row");

    let row = repository::get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "DENIED");

    let event_type = repository::latest_audit_event_type(&repo.handle)
        .await
        .expect("latest_audit_event_type")
        .expect("an audit event exists");
    assert_eq!(
        event_type, "policy.auto_allowed_rejected",
        "a vetoed auto-allowed row must be audited distinctly from a human's \
         rejection of an ordinary pending proposal"
    );
}

/// Unit U54: the CAS still protects against a wrong/already-terminal
/// status — rejecting an already-`DENIED` row must fail, not silently
/// re-apply or panic.
#[tokio::test]
async fn rejecting_an_already_denied_action_fails_not_a_panic() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let now = Utc::now();
    let validated = validate(needs_approval_request(), &registry).expect("validate");
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::PendingApproval { row_id, .. } => row_id,
        other => panic!("expected PendingApproval, got {other:?}"),
    };

    approval::reject(&repo.handle, row_id, "reviewer", now)
        .await
        .expect("first reject should succeed");

    let second = approval::reject(&repo.handle, row_id, "reviewer-again", now).await;
    assert!(
        second.is_err(),
        "rejecting an already-DENIED row a second time must fail"
    );
}
