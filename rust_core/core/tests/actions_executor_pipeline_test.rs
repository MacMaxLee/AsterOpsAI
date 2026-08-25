//! Full, real end-to-end run of TRS §26's nine-stage pipeline: propose ->
//! grant -> authorize -> execute against a real spawned child process, plus
//! the target-identity-mismatch case that must abort before `apply()` ever
//! runs (TRS §27). Integration tests are already test-only code; the
//! workspace's unwrap/expect deny targets production code paths, not
//! `tests/*.rs`.
//!
//! These tests are Linux-only because they rely on `/proc/[pid]/stat` to
//! verify process identity (start time ticks). macOS uses different APIs
//! for process identity verification.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::actions::execute;
use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, Environment, ProtectedResourceRegistry,
    ResourceDescriptor, ResourceKind,
};
use ai_ops_core::repository::get_action;
use chrono::Utc;
use common::{test_registry, AlwaysMismatchVerifier, ProcessTargetVerifier, SpawnedTarget};
use repo_common::TestRepo;

fn kill_request(target: &SpawnedTarget) -> ActionRequest {
    ActionRequest {
        action_type: "test.kill_process".to_string(),
        target: target.target(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Process,
            name: "sleep-test-target".to_string(),
        },
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
    }
}

#[tokio::test]
async fn full_pipeline_kills_a_real_process() {
    let repo = TestRepo::open();
    let registry = test_registry();
    let target = SpawnedTarget::spawn();
    assert!(target.is_alive());

    let request = kill_request(&target);
    let validated = validate(request, &registry).expect("schema validation");

    let now = Utc::now();
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("policy evaluation");
    let row_id = match outcome {
        ai_ops_core::policy::PolicyOutcome::PendingApproval { row_id, .. } => row_id,
        other => panic!("expected PendingApproval for High risk, got {other:?}"),
    };

    approval::grant(&repo.handle, row_id, "approver", Utc::now())
        .await
        .expect("grant");

    let approved = approval::authorize(
        &repo.handle,
        row_id,
        &target.target(),
        &serde_json::json!({}),
        &ResourceDescriptor {
            kind: ResourceKind::Process,
            name: "sleep-test-target".to_string(),
        },
        &registry,
        &common::test_action_context(),
        Utc::now(),
    )
    .await
    .expect("authorize");

    let executed = execute(
        approved,
        &ProcessTargetVerifier,
        &ProtectedResourceRegistry::new(),
        &repo.handle,
    )
    .await
    .expect("execute");

    assert!(!target.is_alive(), "the real target process must be dead");
    assert_eq!(executed.row_id(), row_id);

    let row = get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "EXECUTED");
    assert!(row.executed_at.is_some());

    target.ensure_reaped();
}

#[tokio::test]
async fn target_identity_mismatch_aborts_before_apply() {
    let repo = TestRepo::open();
    let registry = test_registry();
    let target = SpawnedTarget::spawn();

    let request = kill_request(&target);
    let validated = validate(request, &registry).expect("schema validation");

    let now = Utc::now();
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        now,
    )
    .await
    .expect("policy evaluation");
    let row_id = match outcome {
        ai_ops_core::policy::PolicyOutcome::PendingApproval { row_id, .. } => row_id,
        other => panic!("expected PendingApproval for High risk, got {other:?}"),
    };

    approval::grant(&repo.handle, row_id, "approver", Utc::now())
        .await
        .expect("grant");

    let approved = approval::authorize(
        &repo.handle,
        row_id,
        &target.target(),
        &serde_json::json!({}),
        &ResourceDescriptor {
            kind: ResourceKind::Process,
            name: "sleep-test-target".to_string(),
        },
        &registry,
        &common::test_action_context(),
        Utc::now(),
    )
    .await
    .expect("authorize");

    let result = execute(
        approved,
        &AlwaysMismatchVerifier,
        &ProtectedResourceRegistry::new(),
        &repo.handle,
    )
    .await;

    assert!(matches!(
        result,
        Err(ai_ops_core::actions::ActionError::TargetChanged)
    ));
    assert!(
        target.is_alive(),
        "apply() must never have run — the target must still be alive"
    );

    let row = get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "FAILED");

    target.ensure_reaped();
}
