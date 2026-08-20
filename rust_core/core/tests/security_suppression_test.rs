//! Real coverage of SRS FR-SEC-005: a suppressed (detector_id, resource)
//! pair makes `security::record_event` write nothing at all, and the
//! suppression's own creation leaves a real `audit_events` row.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::policy::{ResourceDescriptor, ResourceKind};
use ai_ops_core::repository::reader;
use ai_ops_core::security::{incident, suppress, DetectedEvent, Severity};
use chrono::Utc;
use repo_common::TestRepo;

const DETECTOR_ID: &str = "test.detector";

fn resource(name: &str) -> ResourceDescriptor {
    ResourceDescriptor {
        kind: ResourceKind::Device,
        name: name.to_string(),
    }
}

fn event(resource_name: &str) -> DetectedEvent {
    DetectedEvent {
        detector_id: DETECTOR_ID,
        severity: Severity::Medium,
        category: "TEST",
        summary: "a test event".to_string(),
        evidence_json: "{}".to_string(),
        resource: resource(resource_name),
        ts: Utc::now(),
    }
}

fn security_event_count(repo: &TestRepo) -> i64 {
    let conn = reader::checkout(&repo.handle.read_pool).expect("checkout");
    conn.query_row("SELECT COUNT(*) FROM security_events", [], |row| row.get(0))
        .expect("count security_events")
}

fn audit_event_count_of_type(repo: &TestRepo, event_type: &str) -> i64 {
    let conn = reader::checkout(&repo.handle.read_pool).expect("checkout");
    conn.query_row(
        "SELECT COUNT(*) FROM audit_events WHERE event_type = ?1",
        [event_type],
        |row| row.get(0),
    )
    .expect("count audit_events")
}

#[tokio::test]
async fn a_suppressed_rule_and_resource_writes_nothing() {
    let repo = TestRepo::open();
    let now = Utc::now();

    suppress(
        &repo.handle,
        DETECTOR_ID,
        Some(&resource("sdb")),
        "known false positive on this device",
        "tester",
        now,
    )
    .await
    .expect("suppress");

    let outcome = incident::record_event(&repo.handle, event("sdb"))
        .await
        .expect("record_event");
    assert!(matches!(outcome, incident::IncidentOutcome::Suppressed));
    assert_eq!(security_event_count(&repo), 0);

    // A different resource under the same detector_id is NOT suppressed —
    // this suppression was scoped to "sdb" specifically.
    let other = incident::record_event(&repo.handle, event("sdc"))
        .await
        .expect("record_event");
    assert!(matches!(other, incident::IncidentOutcome::Recorded { .. }));
    assert_eq!(security_event_count(&repo), 1);

    assert_eq!(audit_event_count_of_type(&repo, "security.suppressed"), 1);
}

#[tokio::test]
async fn suppressing_everywhere_blocks_every_resource() {
    let repo = TestRepo::open();
    let now = Utc::now();

    suppress(
        &repo.handle,
        DETECTOR_ID,
        None,
        "this detector is too noisy for now",
        "tester",
        now,
    )
    .await
    .expect("suppress");

    for resource_name in ["sdb", "sdc", "sdd"] {
        let outcome = incident::record_event(&repo.handle, event(resource_name))
            .await
            .expect("record_event");
        assert!(matches!(outcome, incident::IncidentOutcome::Suppressed));
    }
    assert_eq!(security_event_count(&repo), 0);
}
