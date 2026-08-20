//! Real, full-chain coverage of SRS FR-SEC-003, unit U11: a real spawned
//! process, a real `OPEN` incident opened against its `ResourceDescriptor`
//! (via `security::incident::record_event`, bypassing the detectors
//! themselves — this test is about the override rule, not detection), then
//! `host.set_process_priority` through the real, unmodified U7/U8
//! pipeline is rejected with `ActionError::SecurityFlagged` at
//! `actions::executor::execute`'s own stage, while
//! `security.suspend_process` against the *same* flagged target executes
//! for real (`SIGSTOP`, verified via `PlatformAdapter::is_process_stopped`)
//! and rolls back for real (`SIGCONT`).
//!
//! Linux-only: both `actions::host` and `security::response` are gated to
//! `target_os = "linux"` (see their own module docs).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::actions::host::{set_process_priority_entry, ProcessTargetVerifier};
use ai_ops_core::actions::{execute, rollback as rollback_action, ActionError};
use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, ActionTypeRegistry, Environment, PolicyOutcome,
    ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use ai_ops_core::security::response::suspend_process_entry;
use ai_ops_core::security::{incident, DetectedEvent, Severity};
use chrono::Utc;
use common::{test_action_context, SpawnedTarget};
use platform::ProcessPriority;
use repo_common::TestRepo;

fn registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register("host.set_process_priority", set_process_priority_entry());
    registry.register("security.suspend_process", suspend_process_entry());
    registry
}

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

#[tokio::test]
async fn a_flagged_resource_blocks_a_performance_action_but_allows_a_security_response() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();
    let protected = ProtectedResourceRegistry::new();

    // A real OPEN incident against this exact target's resource.
    incident::record_event(
        &repo.handle,
        DetectedEvent {
            detector_id: "test.detector",
            severity: Severity::High,
            category: "TEST",
            summary: "test-induced incident".to_string(),
            evidence_json: "{}".to_string(),
            resource: resource(),
            ts: Utc::now(),
        },
    )
    .await
    .expect("record_event");

    // A performance action against the SAME flagged resource is blocked.
    let priority_request = ActionRequest {
        action_type: "host.set_process_priority".to_string(),
        target: target.target(),
        resource: resource(),
        parameters: serde_json::json!({ "priority": "BELOW_NORMAL" }),
        requested_by: "tester".to_string(),
    };
    let validated = validate(priority_request.clone(), &registry).expect("validate");
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
        other => panic!("expected AutoAllowed for Low risk in Development, got {other:?}"),
    };
    let approved = approval::authorize(
        &repo.handle,
        row_id,
        &priority_request.target,
        &priority_request.parameters,
        &priority_request.resource,
        &registry,
        &context,
        now,
    )
    .await
    .expect("authorize");

    let result = execute(approved, &ProcessTargetVerifier, &protected, &repo.handle).await;
    match result {
        Err(ActionError::SecurityFlagged(_)) => {}
        Err(other) => panic!("expected SecurityFlagged, got a different error: {other:?}"),
        Ok(_) => panic!("expected SecurityFlagged, but execute() succeeded"),
    }

    let priority_after = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after the blocked attempt");
    assert_ne!(
        priority_after,
        ProcessPriority::BelowNormal,
        "a blocked action must never have touched live state"
    );

    // A security-response action against the SAME flagged resource IS
    // allowed — it is never blocked by its own resource's flag.
    let suspend_request = ActionRequest {
        action_type: "security.suspend_process".to_string(),
        target: target.target(),
        resource: resource(),
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
    };
    let validated = validate(suspend_request.clone(), &registry).expect("validate");
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
        other => panic!(
            "expected PendingApproval — TRS §37: security responses always require \
             approval — got {other:?}"
        ),
    };
    approval::grant(&repo.handle, row_id, "approver", now)
        .await
        .expect("grant");
    let approved = approval::authorize(
        &repo.handle,
        row_id,
        &suspend_request.target,
        &suspend_request.parameters,
        &suspend_request.resource,
        &registry,
        &context,
        now,
    )
    .await
    .expect("authorize");

    let executed = execute(approved, &ProcessTargetVerifier, &protected, &repo.handle)
        .await
        .expect("a security response action must not be blocked by its own resource's flag");

    let stopped = context
        .platform
        .is_process_stopped(target.pid)
        .expect("read run state after suspend");
    assert!(
        stopped,
        "SuspendProcessAction must have really stopped the process"
    );

    rollback_action(&executed, &ProcessTargetVerifier, "tester", &repo.handle)
        .await
        .expect("rollback (resume)");
    let stopped_after_rollback = context
        .platform
        .is_process_stopped(target.pid)
        .expect("read run state after rollback");
    assert!(
        !stopped_after_rollback,
        "rollback must have really resumed the process"
    );

    target.ensure_reaped();
}
