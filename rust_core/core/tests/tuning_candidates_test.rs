//! `tuning::candidates::build_candidates` against a real spawned process:
//! only fields that actually differ from the desired state ever produce a
//! candidate, an identical desired state produces nothing. Linux-only,
//! same reasoning as `actions_host_priority_affinity_test.rs`.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "policy/common/mod.rs"]
mod common;

use ai_ops_core::policy::{ResourceDescriptor, ResourceKind};
use ai_ops_core::tuning::candidates::build_candidates;
use ai_ops_core::tuning::DesiredState;
use common::{test_action_context, SpawnedTarget};
use platform::ProcessPriority;

fn resource() -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Process,
        name: "sleep-test-target".to_string(),
    }
}

#[tokio::test]
async fn a_desired_state_identical_to_live_state_proposes_nothing() {
    let target = SpawnedTarget::spawn();
    let context = test_action_context();
    let current_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority");
    let current_affinity = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity");

    let desired = DesiredState {
        priority: Some(current_priority),
        cpu_affinity: Some(current_affinity),
    };

    let candidates = build_candidates(&desired, target.target(), &resource(), &context, "tester")
        .expect("build_candidates");
    assert!(
        candidates.is_empty(),
        "identical desired state must propose nothing, got {candidates:?}"
    );

    target.ensure_reaped();
}

#[tokio::test]
async fn only_the_differing_field_proposes_a_candidate() {
    let target = SpawnedTarget::spawn();
    let context = test_action_context();
    let current_priority = context
        .platform
        .get_process_priority(target.pid)
        .expect("read priority");
    let current_affinity = context
        .platform
        .get_process_cpu_affinity(target.pid)
        .expect("read affinity");

    // Lowering priority (raising the nice value) is always achievable
    // unprivileged (docs/adr/0013) — a safe, deterministic way to force a
    // real difference without needing any elevated capability.
    let lowered = if current_priority == ProcessPriority::Idle {
        ProcessPriority::BelowNormal
    } else {
        ProcessPriority::Idle
    };
    let desired = DesiredState {
        priority: Some(lowered),
        cpu_affinity: Some(current_affinity),
    };

    let candidates = build_candidates(&desired, target.target(), &resource(), &context, "tester")
        .expect("build_candidates");
    assert_eq!(
        candidates.len(),
        1,
        "only the differing field should propose a candidate, got {candidates:?}"
    );
    assert_eq!(candidates[0].action_type, "host.set_process_priority");

    target.ensure_reaped();
}
