//! Real coverage of SRS FR-SEC-002's correlation rule
//! (`repository::security::insert_event`, via `security::record_event`):
//! two events from the same detector against the same resource land in
//! the same `OPEN` incident; a different resource opens a second one.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::policy::{ResourceDescriptor, ResourceKind};
use ai_ops_core::security::{incident, DetectedEvent, Severity};
use chrono::Utc;
use repo_common::TestRepo;

fn event(resource_name: &str) -> DetectedEvent {
    event_with_severity(resource_name, Severity::Medium)
}

fn event_with_severity(resource_name: &str, severity: Severity) -> DetectedEvent {
    DetectedEvent {
        detector_id: "test.detector",
        severity,
        category: "TEST",
        summary: "a test event".to_string(),
        evidence_json: "{}".to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Device,
            name: resource_name.to_string(),
        },
        ts: Utc::now(),
    }
}

fn incident_id(outcome: &incident::IncidentOutcome) -> i64 {
    match outcome {
        incident::IncidentOutcome::Recorded { incident_id, .. } => *incident_id,
        other => panic!("expected Recorded, got {other:?}"),
    }
}

#[tokio::test]
async fn same_detector_and_resource_correlate_into_one_incident() {
    let repo = TestRepo::open();

    let first = incident::record_event(&repo.handle, event("sdb"))
        .await
        .expect("record_event");
    let second = incident::record_event(&repo.handle, event("sdb"))
        .await
        .expect("record_event");

    assert_eq!(
        incident_id(&first),
        incident_id(&second),
        "a second event from the same detector against the same resource must \
         reuse the open incident, not open a new one"
    );
}

/// Unit U75 (ADR 0065's own "escalate severity" remainder): an
/// incident's own severity used to be fixed forever at whichever event
/// happened to open it. A later, more severe correlated event now
/// raises it.
#[tokio::test]
async fn a_later_more_severe_correlated_event_raises_the_incidents_own_severity() {
    let repo = TestRepo::open();

    incident::record_event(&repo.handle, event_with_severity("sdb", Severity::Medium))
        .await
        .expect("record_event");
    let second = incident::record_event(&repo.handle, event_with_severity("sdb", Severity::High))
        .await
        .expect("record_event");

    let incident_id = incident_id(&second);
    let conn = ai_ops_core::repository::reader::checkout(&repo.handle.read_pool).expect("checkout");
    let severity: String = conn
        .query_row(
            "SELECT severity FROM security_incidents WHERE id = ?1",
            [incident_id],
            |row| row.get(0),
        )
        .expect("query incident severity");
    assert_eq!(
        severity, "HIGH",
        "a later, more severe correlated event must raise the incident's own severity"
    );
}

/// Mirrors the escalation test above: a later, *milder* correlated
/// event must never lower an incident's severity back down.
#[tokio::test]
async fn a_later_milder_correlated_event_never_lowers_the_incidents_own_severity() {
    let repo = TestRepo::open();

    incident::record_event(&repo.handle, event_with_severity("sdb", Severity::High))
        .await
        .expect("record_event");
    let second = incident::record_event(&repo.handle, event_with_severity("sdb", Severity::Low))
        .await
        .expect("record_event");

    let incident_id = incident_id(&second);
    let conn = ai_ops_core::repository::reader::checkout(&repo.handle.read_pool).expect("checkout");
    let severity: String = conn
        .query_row(
            "SELECT severity FROM security_incidents WHERE id = ?1",
            [incident_id],
            |row| row.get(0),
        )
        .expect("query incident severity");
    assert_eq!(
        severity, "HIGH",
        "an incident that was ever HIGH must stay at least HIGH, never demoted by a milder event"
    );
}

/// Unit U76 (ADR 0016/0020's own named gap, resolved to manual-only —
/// docs/adr/0081): the repository-level `close_security_incident`
/// itself, distinct from `security_endpoints.rs`'s own HTTP-level
/// proof of the same behavior.
#[tokio::test]
async fn close_security_incident_marks_it_closed_with_a_real_timestamp() {
    let repo = TestRepo::open();
    let recorded = incident::record_event(&repo.handle, event("sdb"))
        .await
        .expect("record_event");
    let id = incident_id(&recorded);

    // `closed_at` is persisted with millisecond precision (`repository::
    // time::format_ts`), so compare against a millisecond-truncated
    // `now`, not `Utc::now()`'s own sub-millisecond value.
    let now = Utc::now();
    let detail = ai_ops_core::repository::close_security_incident(&repo.handle, id, now)
        .await
        .expect("close_security_incident")
        .expect("incident exists");
    assert_eq!(detail.incident.status, "CLOSED");
    assert_eq!(
        detail
            .incident
            .closed_at
            .expect("closed_at is set")
            .timestamp_millis(),
        now.timestamp_millis()
    );
    assert_eq!(detail.event_count, 1);
    assert_eq!(detail.detector_id, "test.detector");
    assert_eq!(detail.resource_kind, "DEVICE");
    assert_eq!(detail.resource_name, "sdb");
}

#[tokio::test]
async fn close_security_incident_on_an_unknown_id_returns_none() {
    let repo = TestRepo::open();
    let result =
        ai_ops_core::repository::close_security_incident(&repo.handle, 999_999, Utc::now())
            .await
            .expect("close_security_incident");
    assert!(
        result.is_none(),
        "an unknown incident id must return None, not an error"
    );
}

#[tokio::test]
async fn a_different_resource_opens_a_distinct_incident() {
    let repo = TestRepo::open();

    let first = incident::record_event(&repo.handle, event("sdb"))
        .await
        .expect("record_event");
    let second = incident::record_event(&repo.handle, event("sdc"))
        .await
        .expect("record_event");

    assert_ne!(
        incident_id(&first),
        incident_id(&second),
        "events against genuinely different resources must not be correlated together"
    );
}
