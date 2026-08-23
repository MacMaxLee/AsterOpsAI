//! Real coverage of unit U28's `rollback_by_row_id` — the only way to
//! reach `rollback()` for a row that wasn't just executed in the same
//! call chain. Every prior rollback test in this crate
//! (`actions_host_priority_affinity_test.rs`, `security_flag_overrides_
//! performance_action_test.rs`) calls `execute()` then `rollback()`
//! back-to-back with the same in-memory `Executed` — this file is the
//! first to reconstruct one from a stored row looked up *separately*,
//! the exact shape a real HTTP rollback endpoint needs.
//!
//! Linux-only: both `actions::host` and `security::response` are gated
//! to `target_os = "linux"` (see their own module docs).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::actions::host::{
    set_process_cpu_affinity_entry, set_process_priority_entry, ProcessTargetVerifier,
};
use ai_ops_core::actions::{execute, rollback_by_row_id, ActionError};
use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, ActionTypeRegistry, Environment, PolicyError,
    PolicyOutcome, ProtectedResourceRegistry, ResourceDescriptor, ResourceKind,
};
use ai_ops_core::security::response::suspend_process_entry;
use chrono::Utc;
use common::{test_action_context, AlwaysMismatchVerifier, SpawnedTarget};
use repo_common::TestRepo;

fn registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register("host.set_process_priority", set_process_priority_entry());
    registry.register(
        "host.set_process_cpu_affinity",
        set_process_cpu_affinity_entry(),
    );
    registry.register("security.suspend_process", suspend_process_entry());
    registry
}

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

/// Proposes, evaluates, grants-if-needed, authorizes, and really
/// executes `action_type` against `target`, returning the real
/// `EXECUTED` row's id — mirrors `security_flag_overrides_performance_
/// action_test.rs`'s own chain, generalized over both `AutoAllowed` and
/// `PendingApproval` outcomes (Low risk auto-allows in `Development`,
/// `security.suspend_process`'s `High` risk never does).
async fn execute_for_real(
    repo: &TestRepo,
    registry: &ActionTypeRegistry,
    action_type: &str,
    target: &SpawnedTarget,
    parameters: serde_json::Value,
) -> i64 {
    let context = test_action_context();
    let request = ActionRequest {
        action_type: action_type.to_string(),
        target: target.target(),
        resource: resource(),
        parameters: parameters.clone(),
        requested_by: "tester".to_string(),
    };
    let validated = validate(request.clone(), registry).expect("validate");
    let now = Utc::now();
    let outcome = evaluate(
        validated,
        Environment::Development,
        registry,
        &repo.handle,
        now,
    )
    .await
    .expect("evaluate");
    let row_id = match outcome {
        PolicyOutcome::AutoAllowed { row_id } => row_id,
        PolicyOutcome::PendingApproval { row_id, .. } => {
            approval::grant(&repo.handle, row_id, "approver", now)
                .await
                .expect("grant");
            row_id
        }
        other => panic!("expected AutoAllowed or PendingApproval, got {other:?}"),
    };
    let approved = approval::authorize(
        &repo.handle,
        row_id,
        &request.target,
        &request.parameters,
        &request.resource,
        registry,
        &context,
        now,
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
    executed.row_id()
}

/// The capstone: a real process is really suspended (via the full
/// propose/evaluate/grant/authorize/execute chain, unit U27's own real
/// path), then — in a genuinely separate call, knowing only the row id,
/// nothing else from the original chain — `rollback_by_row_id` really
/// resumes it. This is the exact gap found after U27: nothing could do
/// this before this unit.
///
/// SRS FR-BENCH-003: rollback re-verifies target identity via a real
/// verifier before acting.
#[tokio::test]
async fn rollback_by_row_id_really_resumes_a_really_suspended_process() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let row_id = execute_for_real(
        &repo,
        &registry,
        "security.suspend_process",
        &target,
        serde_json::json!({}),
    )
    .await;

    let stopped = context
        .platform
        .is_process_stopped(target.pid)
        .expect("is_process_stopped after suspend");
    assert!(stopped, "the real process must be stopped after execute()");

    // Only the row id crosses into this call — no `Executed` handle
    // survives from `execute_for_real` above.
    rollback_by_row_id(
        row_id,
        &registry,
        &context,
        &ProcessTargetVerifier,
        "tester",
        &repo.handle,
    )
    .await
    .expect("rollback_by_row_id");

    let stopped_after = context
        .platform
        .is_process_stopped(target.pid)
        .expect("is_process_stopped after rollback");
    assert!(
        !stopped_after,
        "the real process must be resumed after rollback_by_row_id — the exact \
         capability that was missing before this unit"
    );

    target.ensure_reaped();
}

/// SRS FR-BENCH-003, the refusal half: every other rollback test here
/// only exercises the success path (identity matches). Mirrors
/// `actions_executor_pipeline_test.rs`'s own
/// `target_identity_mismatch_aborts_before_apply` for the forward path —
/// `rollback()`'s own `verifier.verify(...)?` is the very first thing it
/// does, before anything is persisted, so a mismatch here must leave
/// the real process exactly as `execute()` left it (still stopped) and
/// the row's own status untouched (`EXECUTED`, not `ROLLED_BACK`).
#[tokio::test]
async fn rollback_by_row_id_refuses_when_target_identity_has_changed() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let row_id = execute_for_real(
        &repo,
        &registry,
        "security.suspend_process",
        &target,
        serde_json::json!({}),
    )
    .await;

    let stopped = context
        .platform
        .is_process_stopped(target.pid)
        .expect("is_process_stopped after suspend");
    assert!(stopped, "the real process must be stopped after execute()");

    let result = rollback_by_row_id(
        row_id,
        &registry,
        &context,
        &AlwaysMismatchVerifier,
        "tester",
        &repo.handle,
    )
    .await;

    assert!(matches!(result, Err(ActionError::TargetChanged)));

    let stopped_after = context
        .platform
        .is_process_stopped(target.pid)
        .expect("is_process_stopped after refused rollback");
    assert!(
        stopped_after,
        "rollback() must never have run — the process must still be stopped"
    );

    let row = ai_ops_core::repository::get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(
        row.status, "EXECUTED",
        "the original row's status must be untouched by a refused rollback"
    );

    target.ensure_reaped();
}

/// `host.set_process_cpu_affinity` is `reversible: true` (unit U10's own
/// flag). Unit U30 (ADR 0033/0035) persists the real `previous_state`
/// `execute()` captures (migrations/V13), so `rollback_by_row_id` now
/// genuinely restores it — real proof, not just "it fails safely" the
/// way this test asserted before that migration existed.
#[tokio::test]
async fn rollback_by_row_id_of_an_affinity_change_really_restores_the_real_previous_value() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let original = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read original affinity");

    let row_id = execute_for_real(
        &repo,
        &registry,
        "host.set_process_cpu_affinity",
        &target,
        serde_json::json!({ "cpus": [0] }),
    )
    .await;

    let after_apply = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity after apply");
    assert_eq!(after_apply.cpus, std::collections::BTreeSet::from([0]));

    rollback_by_row_id(
        row_id,
        &registry,
        &context,
        &ProcessTargetVerifier,
        "tester",
        &repo.handle,
    )
    .await
    .expect("rollback_by_row_id");

    let after_rollback = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity after rollback");
    assert_eq!(
        after_rollback, original,
        "rollback_by_row_id must restore the real, historical previous \
         value now that it's actually persisted (unit U30), not just fail safely"
    );

    target.ensure_reaped();
}

/// `host.set_process_priority` is `reversible: false` (unit U10's own
/// flag — ADR 0013's real `CAP_SYS_NICE` asymmetry finding) — the new
/// pre-flight guard refuses outright, never even reaching `rollback()`.
#[tokio::test]
async fn rollback_by_row_id_of_a_non_reversible_action_type_is_refused_outright() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let row_id = execute_for_real(
        &repo,
        &registry,
        "host.set_process_priority",
        &target,
        serde_json::json!({ "priority": "BELOW_NORMAL" }),
    )
    .await;

    let result = rollback_by_row_id(
        row_id,
        &registry,
        &context,
        &ProcessTargetVerifier,
        "tester",
        &repo.handle,
    )
    .await;

    match result {
        Err(ActionError::CapabilityUnavailable(msg)) => {
            assert!(msg.contains("not reversible"), "got: {msg}");
        }
        other => panic!("expected a real CapabilityUnavailable refusal, got {other:?}"),
    }

    target.ensure_reaped();
}

#[tokio::test]
async fn rollback_by_row_id_of_a_never_executed_row_is_a_real_wrong_status_error() {
    let repo = TestRepo::open();
    let registry = registry();
    let target = SpawnedTarget::spawn();
    let context = test_action_context();

    let request = ActionRequest {
        action_type: "host.set_process_cpu_affinity".to_string(),
        target: target.target(),
        resource: resource(),
        parameters: serde_json::json!({ "cpus": [0] }),
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
        PolicyOutcome::AutoAllowed { row_id } => row_id,
        other => panic!("expected AutoAllowed, got {other:?}"),
    };

    let result = rollback_by_row_id(
        row_id,
        &registry,
        &context,
        &ProcessTargetVerifier,
        "tester",
        &repo.handle,
    )
    .await;

    match result {
        Err(ActionError::Policy(PolicyError::WrongStatus {
            id,
            expected,
            actual,
        })) => {
            assert_eq!(id, row_id);
            assert_eq!(expected, "EXECUTED");
            assert_eq!(actual, "AUTO_ALLOWED");
        }
        other => panic!("expected a real WrongStatus error, got {other:?}"),
    }

    target.ensure_reaped();
}

#[tokio::test]
async fn rollback_by_row_id_of_a_nonexistent_row_is_a_real_not_found_error() {
    let repo = TestRepo::open();
    let registry = registry();
    let context = test_action_context();

    let result = rollback_by_row_id(
        999_999,
        &registry,
        &context,
        &ProcessTargetVerifier,
        "tester",
        &repo.handle,
    )
    .await;

    match result {
        Err(ActionError::Policy(PolicyError::NotFound(id))) => assert_eq!(id, 999_999),
        other => panic!("expected a real NotFound error, got {other:?}"),
    }
}
