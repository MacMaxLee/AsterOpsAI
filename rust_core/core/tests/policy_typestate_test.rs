//! TRS §24's typestate chain: `Validated<T>`/`PolicyApproved` have private
//! inner fields and `pub(in crate::policy)` constructors — a policy bypass
//! (building either by hand, skipping `validate()`/`approval::authorize()`)
//! is a **compile error**, not something expressible as a failing runtime
//! assertion. This file can only demonstrate the *positive* path — that
//! going through the real functions actually produces a usable value — the
//! negative guarantee is what `cargo build` already proves every time this
//! crate compiles: nothing outside `core::policy` has ever been able to
//! write `Validated { inner: ... }` or `PolicyApproved { action: ... }`
//! literal syntax, since those fields aren't `pub`.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::policy::{
    approval, evaluate, validate, ActionRequest, Environment, PolicyOutcome, ResourceDescriptor,
    ResourceKind, TargetIdentity,
};
use chrono::Utc;
use common::lifecycle_test_registry;
use repo_common::TestRepo;

#[test]
fn validate_is_the_only_way_to_get_a_validated_action_request() {
    let registry = lifecycle_test_registry();
    let request = ActionRequest {
        action_type: "test.auto_allow".to_string(),
        target: TargetIdentity::Process {
            pid: 1,
            start_time_ticks: 1,
        },
        resource: ResourceDescriptor {
            kind: ResourceKind::Process,
            name: "dummy".to_string(),
        },
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
    };
    let validated = validate(request, &registry).expect("validate");
    // `.get()`/`.into_inner()` are the only ways to read the wrapped value
    // back out — both public, neither exposes the field itself.
    assert_eq!(validated.get().action_type, "test.auto_allow");
    let inner = validated.into_inner();
    assert_eq!(inner.requested_by, "tester");
}

#[tokio::test]
async fn authorize_is_the_only_way_to_get_a_policy_approved_value() {
    let repo = TestRepo::open();
    let registry = lifecycle_test_registry();
    let request = ActionRequest {
        action_type: "test.auto_allow".to_string(),
        target: TargetIdentity::Process {
            pid: 1,
            start_time_ticks: 1,
        },
        resource: ResourceDescriptor {
            kind: ResourceKind::Process,
            name: "dummy".to_string(),
        },
        parameters: serde_json::json!({}),
        requested_by: "tester".to_string(),
    };
    let validated = validate(request.clone(), &registry).expect("validate");
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
        &request.target,
        &request.parameters,
        &request.resource,
        &registry,
        now,
    )
    .await
    .expect("authorize");

    // `.action()`/`.row_id()`/`.target()`/`.resource()` are read-only
    // accessors — the executor consumes `approved` by value, but nothing
    // outside `core::policy` can construct one from scratch.
    assert_eq!(approved.row_id(), row_id);
    assert_eq!(approved.action().action_type(), "test.no_op");
}
