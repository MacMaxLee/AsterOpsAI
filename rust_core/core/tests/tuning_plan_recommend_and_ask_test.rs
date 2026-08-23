//! `AutomationMode::RecommendOnly`/`AskBeforeChanges` scope boundary,
//! real end to end: `RecommendOnly` never touches the policy engine at all
//! (no new `actions` rows); `AskBeforeChanges` creates real, audited
//! proposals but never authorizes them, regardless of what the policy
//! decision would have allowed — live process state never changes either
//! way. Linux-only, same reasoning as
//! `actions_host_priority_affinity_test.rs`.
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
use ai_ops_core::policy::{
    ActionStatus, ActionTypeRegistry, Environment, ProtectedResourceRegistry, ResourceDescriptor,
    ResourceKind,
};
use ai_ops_core::repository::get_action;
use ai_ops_core::tuning::TuningPipeline;
use ai_ops_core::tuning::{start_plan, AutomationMode, TuningProfile};
use chrono::Utc;
use common::{test_action_context, SpawnedTarget};
use repo_common::TestRepo;

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

fn registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register("host.set_process_priority", set_process_priority_entry());
    registry
}

fn action_count(repo: &TestRepo) -> i64 {
    let conn = ai_ops_core::repository::reader::checkout(&repo.handle.read_pool).expect("checkout");
    conn.query_row("SELECT COUNT(*) FROM actions", [], |row| row.get(0))
        .expect("count actions")
}

#[tokio::test]
async fn recommend_only_never_touches_the_policy_engine() {
    let repo = TestRepo::open();
    let registry = registry();
    let context = test_action_context();
    let protected = ProtectedResourceRegistry::new();
    let target = SpawnedTarget::spawn();
    let original_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read original priority");

    let pipeline = TuningPipeline {
        handle: &repo.handle,
        registry: &registry,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        environment: Environment::Development,
    };

    let before = action_count(&repo);

    let outcome = start_plan(
        target.target(),
        resource(),
        TuningProfile::BatterySaver,
        AutomationMode::RecommendOnly,
        "tester",
        &pipeline,
        Utc::now(),
    )
    .await
    .expect("start_plan");

    assert!(
        !outcome.candidates.is_empty(),
        "BatterySaver against a freshly spawned (Normal) process must propose something"
    );
    for candidate in &outcome.candidates {
        assert_eq!(candidate.outcome, "RECOMMENDED");
        assert_eq!(candidate.row_id, None);
    }
    assert_eq!(
        action_count(&repo),
        before,
        "RECOMMEND_ONLY must never create an actions row"
    );

    let live_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after the plan");
    assert_eq!(
        live_priority, original_priority,
        "RECOMMEND_ONLY must never touch live state"
    );

    target.ensure_reaped();
}

/// SRS FR-TUNE-001: a profile's candidates are proposed for real but
/// realized only through policy, never auto-authorized.
#[tokio::test]
async fn ask_before_changes_proposes_for_real_but_never_authorizes() {
    let repo = TestRepo::open();
    let registry = registry();
    let context = test_action_context();
    let protected = ProtectedResourceRegistry::new();
    let target = SpawnedTarget::spawn();
    let original_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read original priority");

    let pipeline = TuningPipeline {
        handle: &repo.handle,
        registry: &registry,
        context: &context,
        verifier: &ProcessTargetVerifier,
        protected: &protected,
        environment: Environment::Development,
    };

    let before = action_count(&repo);

    let outcome = start_plan(
        target.target(),
        resource(),
        TuningProfile::BatterySaver,
        AutomationMode::AskBeforeChanges,
        "tester",
        &pipeline,
        Utc::now(),
    )
    .await
    .expect("start_plan");

    assert!(!outcome.candidates.is_empty());
    let priority_candidate = outcome
        .candidates
        .iter()
        .find(|c| c.action_type == "host.set_process_priority")
        .expect("a priority candidate must have been proposed");
    // Low risk in Development auto-allows per policy::risk::decide — but
    // ASK_BEFORE_CHANGES must still never authorize it.
    assert_eq!(priority_candidate.outcome, "AUTO_ALLOWED_PENDING");
    let row_id = priority_candidate.row_id.expect("a real actions row id");

    assert!(
        action_count(&repo) > before,
        "ASK_BEFORE_CHANGES must create a real, audited actions row"
    );

    let row = get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(
        row.status,
        ActionStatus::AutoAllowed.as_str(),
        "the row must never advance past its proposed status: ASK_BEFORE_CHANGES never calls authorize()"
    );

    let live_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority after the plan");
    assert_eq!(
        live_priority, original_priority,
        "a never-authorized proposal must never touch live state"
    );

    target.ensure_reaped();
}
