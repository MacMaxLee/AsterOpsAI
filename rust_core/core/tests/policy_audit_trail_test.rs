//! SRS FR-POL-005: every policy decision — including denials — is written
//! to the audit log. Integration tests are already test-only code; the
//! workspace's unwrap/expect deny targets production code paths, not
//! `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, Environment, PolicyOutcome, ResourceDescriptor,
    ResourceKind, TargetIdentity,
};
use ai_ops_core::repository::reader;
use chrono::Utc;
use common::lifecycle_test_registry;
use repo_common::TestRepo;

fn audit_event_count(repo: &TestRepo) -> i64 {
    let conn = reader::checkout(&repo.handle.read_pool).expect("checkout");
    conn.query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
        .expect("count audit_events")
}

fn latest_event_type(repo: &TestRepo) -> String {
    let conn = reader::checkout(&repo.handle.read_pool).expect("checkout");
    conn.query_row(
        "SELECT event_type FROM audit_events ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .expect("read latest audit event")
}

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
async fn auto_allowed_decision_is_audited() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let before = audit_event_count(&repo);

    let validated = validate(request("test.auto_allow"), &registry).expect("validate");
    evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        Utc::now(),
    )
    .await
    .expect("evaluate");

    assert!(audit_event_count(&repo) > before);
    assert_eq!(latest_event_type(&repo), "policy.auto_allowed");
}

#[tokio::test]
async fn pending_approval_decision_is_audited() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let before = audit_event_count(&repo);

    let validated = validate(request("test.needs_approval"), &registry).expect("validate");
    evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        Utc::now(),
    )
    .await
    .expect("evaluate");

    assert!(audit_event_count(&repo) > before);
    assert_eq!(latest_event_type(&repo), "policy.pending_approval");
}

#[tokio::test]
async fn denied_decision_is_audited_even_though_the_action_never_ran() {
    let mut registry = lifecycle_test_registry();
    registry.register(
        "test.prohibited",
        ai_ops_core::policy::ActionTypeEntry {
            risk_level: ai_ops_core::policy::RiskLevel::Prohibited,
            validate_parameters: |_| Ok(()),
            construct: |_| unreachable!("a Prohibited action is never constructed"),
        },
    );
    let repo = TestRepo::open();
    let before = audit_event_count(&repo);

    let validated = validate(request("test.prohibited"), &registry).expect("validate");
    let outcome = evaluate(
        validated,
        Environment::Development,
        &registry,
        &repo.handle,
        Utc::now(),
    )
    .await
    .expect("evaluate");
    assert!(matches!(outcome, PolicyOutcome::Denied { .. }));

    assert!(
        audit_event_count(&repo) > before,
        "a denial must still be audited (FR-POL-005)"
    );
    assert_eq!(latest_event_type(&repo), "policy.denied");
}

#[tokio::test]
async fn grant_is_audited() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let validated = validate(request("test.needs_approval"), &registry).expect("validate");
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
        other => panic!("expected PendingApproval, got {other:?}"),
    };

    let before = audit_event_count(&repo);
    approval::grant(&repo.handle, row_id, "approver", now)
        .await
        .expect("grant");

    assert!(audit_event_count(&repo) > before);
    assert_eq!(latest_event_type(&repo), "policy.granted");

    let row = ai_ops_core::repository::get_action(&repo.handle, row_id)
        .await
        .expect("get_action")
        .expect("row exists");
    assert_eq!(row.approved_by.as_deref(), Some("approver"));
}
