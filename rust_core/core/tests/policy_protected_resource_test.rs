//! SRS FR-POL-006: the protected-resource registry is consulted by the
//! executor pipeline before any mutating action — never bypassable at a
//! call site. Integration tests are already test-only code; the
//! workspace's unwrap/expect deny targets production code paths, not
//! `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::actions::{execute, ActionError};
use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, Environment, PolicyOutcome,
    ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use chrono::Utc;
use common::{test_registry, ProcessTargetVerifier, SpawnedTarget};
use repo_common::TestRepo;

#[tokio::test]
async fn protected_resource_blocks_execution_even_after_approval() {
    let repo = TestRepo::open();
    let registry = test_registry();
    let target = SpawnedTarget::spawn();

    let resource = ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    };
    let request = ActionRequest {
        action_type: "test.kill_process".to_string(),
        target: target.target(),
        resource: resource.clone(),
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
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
        PolicyOutcome::PendingApproval { row_id, .. } => row_id,
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
        &resource,
        &registry,
        Utc::now(),
    )
    .await
    .expect("authorize");

    let mut protected = ProtectedResourceRegistry::new();
    protected.protect(resource.clone());

    let result = execute(approved, &ProcessTargetVerifier, &protected, &repo.handle).await;

    assert!(matches!(result, Err(ActionError::Protected(_))));
    assert!(
        target.is_alive(),
        "the protected target must never actually be acted on"
    );

    let row = ai_ops_core::repository::get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.status, "FAILED");

    target.ensure_reaped();
}

#[tokio::test]
async fn an_unprotected_resource_of_the_same_kind_but_different_name_is_unaffected() {
    let repo = TestRepo::open();
    let registry = test_registry();
    let target = SpawnedTarget::spawn();

    let resource = ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    };
    let request = ActionRequest {
        action_type: "test.kill_process".to_string(),
        target: target.target(),
        resource: resource.clone(),
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
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
        PolicyOutcome::PendingApproval { row_id, .. } => row_id,
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
        &resource,
        &registry,
        Utc::now(),
    )
    .await
    .expect("authorize");

    let mut protected = ProtectedResourceRegistry::new();
    protected.protect(ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "some-other-protected-process".to_string(),
    });

    let result = execute(approved, &ProcessTargetVerifier, &protected, &repo.handle).await;
    assert!(result.is_ok());
    assert!(!target.is_alive());
}
