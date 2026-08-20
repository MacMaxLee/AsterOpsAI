//! Real end-to-end coverage of unit U8's two real actions (TRS §38's "the
//! two U8 actions") against a real spawned disposable process: propose ->
//! evaluate (Low risk auto-allows in Development) -> authorize -> execute,
//! confirming the live process's real priority/CPU affinity actually
//! changed (read back via the same `PlatformAdapter` methods, never just
//! "apply() returned Ok"), then a real rollback restoring the original
//! value, then a real permission-denied path (requesting `High` priority
//! as this sandbox's unprivileged test user).
//!
//! Linux-only: `core::actions::host` is gated to `target_os = "linux"`
//! (see its own module doc) since Windows/macOS have no real
//! `PlatformAdapter` implementation of these capabilities yet (unit U12).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use std::collections::BTreeSet;

use ai_ops_core::actions::host::{
    set_process_cpu_affinity_entry, set_process_priority_entry, ProcessTargetVerifier,
};
use ai_ops_core::actions::{execute, rollback as rollback_action};
use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, ActionTypeRegistry, Environment, PolicyOutcome,
    ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use chrono::Utc;
use common::{test_action_context, SpawnedTarget};
use platform::{CpuAffinityMask, ProcessPriority};
use repo_common::TestRepo;

fn registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register("host.set_process_priority", set_process_priority_entry());
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

async fn propose_evaluate_authorize(
    repo: &repo_common::TestRepo,
    registry: &ActionTypeRegistry,
    action_type: &str,
    target: &SpawnedTarget,
    parameters: serde_json::Value,
) -> ai_ops_core::policy::PolicyApproved {
    let request = ActionRequest {
        action_type: action_type.to_string(),
        target: target.target(),
        resource: resource(),
        parameters: parameters.clone(),
        requested_by: "tester".to_string(),
    };
    let validated = validate(request, registry).expect("schema validation");
    let now = Utc::now();
    let outcome = evaluate(
        validated,
        Environment::Development,
        registry,
        &repo.handle,
        now,
    )
    .await
    .expect("policy evaluation");
    let row_id = match outcome {
        PolicyOutcome::AutoAllowed { row_id } => row_id,
        other => panic!("expected AutoAllowed for Low risk in Development, got {other:?}"),
    };
    approval::authorize(
        &repo.handle,
        row_id,
        &target.target(),
        &parameters,
        &resource(),
        registry,
        &test_action_context(),
        now,
    )
    .await
    .expect("authorize")
}

/// A real, important discovery, not a code bug: Linux only ever lets an
/// *unprivileged* process *raise* its own nice value (lower its priority)
/// relative to the current value — never lower it again, even to undo a
/// change it made itself (`setpriority(2)`/`nice(2)`'s documented
/// behavior, needing `CAP_SYS_NICE` to go the other way). So
/// `Normal -> BelowNormal` applies cleanly, but rolling back
/// `BelowNormal -> Normal` needs the same privilege a fresh `Normal ->
/// AboveNormal` would — and fails the same honest way. This test proves
/// that real, asymmetric constraint is surfaced cleanly (a real `Err`,
/// not a panic, not a silent no-op, and the live process's priority stays
/// exactly where `apply()` left it rather than being partially reverted).
#[tokio::test]
async fn rollback_of_a_priority_lowering_action_fails_cleanly_when_unprivileged() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let approved = propose_evaluate_authorize(
        &repo,
        &registry,
        "host.set_process_priority",
        &target,
        serde_json::json!({ "priority": "BELOW_NORMAL" }),
    )
    .await;

    let executed = execute(
        approved,
        &ProcessTargetVerifier,
        &ProtectedResourceRegistry::new(),
        &repo.handle,
    )
    .await
    .expect("execute");

    let after_apply = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after apply");
    assert_eq!(after_apply, ProcessPriority::BelowNormal);

    let rollback_result =
        rollback_action(&executed, &ProcessTargetVerifier, "tester", &repo.handle).await;
    assert!(
        rollback_result.is_err(),
        "restoring a lower nice value needs CAP_SYS_NICE; an unprivileged rollback must fail, not silently no-op"
    );

    let after_failed_rollback = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after the failed rollback");
    assert_eq!(
        after_failed_rollback,
        ProcessPriority::BelowNormal,
        "a failed rollback must leave live state exactly as apply() left it, not partially reverted"
    );

    target.ensure_reaped();
}

#[tokio::test]
async fn set_process_cpu_affinity_end_to_end_with_rollback() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let original = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read original affinity");

    let approved = propose_evaluate_authorize(
        &repo,
        &registry,
        "host.set_process_cpu_affinity",
        &target,
        serde_json::json!({ "cpus": [0] }),
    )
    .await;

    let executed = execute(
        approved,
        &ProcessTargetVerifier,
        &ProtectedResourceRegistry::new(),
        &repo.handle,
    )
    .await
    .expect("execute");

    let after_apply = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity after apply");
    assert_eq!(
        after_apply,
        CpuAffinityMask {
            cpus: BTreeSet::from([0])
        }
    );

    rollback_action(&executed, &ProcessTargetVerifier, "tester", &repo.handle)
        .await
        .expect("rollback");

    let after_rollback = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity after rollback");
    assert_eq!(after_rollback, original);

    target.ensure_reaped();
}

#[tokio::test]
async fn requesting_high_priority_unprivileged_fails_cleanly_without_mutating_state() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let original = context
        .platform
        .get_process_priority(target.pid)
        .expect("read original priority");
    assert_ne!(
        original,
        ProcessPriority::High,
        "test assumes the spawned target does not already run at High priority"
    );

    let approved = propose_evaluate_authorize(
        &repo,
        &registry,
        "host.set_process_priority",
        &target,
        serde_json::json!({ "priority": "HIGH" }),
    )
    .await;

    let result = execute(
        approved,
        &ProcessTargetVerifier,
        &ProtectedResourceRegistry::new(),
        &repo.handle,
    )
    .await;

    assert!(
        result.is_err(),
        "boosting priority without CAP_SYS_NICE must fail, not silently succeed or no-op"
    );

    let unchanged = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after the failed attempt");
    assert_eq!(
        unchanged, original,
        "a failed apply() must never have partially changed live state"
    );

    target.ensure_reaped();
}
