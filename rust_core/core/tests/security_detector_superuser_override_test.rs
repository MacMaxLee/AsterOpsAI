//! Real end-to-end coverage of the superuser-override security detector
//! (SRS FR-SEC-001/002, unit U11): a real PostgreSQL superuser connection,
//! a real `dbms::role_check::validate` call, then
//! `security::detect_superuser_override` + `security::record_event`
//! against a real SQLite-backed repository — fires only when the
//! override was actually used, and lands in a real, queryable
//! `security_incidents`/`security_events` row.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "dbms/common/mod.rs"]
mod dbms_common;
#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::dbms::role_check::validate;
use ai_ops_core::dbms::Environment;
use ai_ops_core::repository::reader;
use ai_ops_core::security::{detect_superuser_override, incident, DETECTOR_SUPERUSER_OVERRIDE};
use chrono::Utc;
use dbms_common::TestPostgres;
use repo_common::TestRepo;

fn security_events_for(repo: &TestRepo, detector_id: &str) -> i64 {
    let conn = reader::checkout(&repo.handle.read_pool).expect("checkout");
    conn.query_row(
        "SELECT COUNT(*) FROM security_events WHERE detector_id = ?1",
        [detector_id],
        |row| row.get(0),
    )
    .expect("count security_events")
}

#[tokio::test]
async fn override_used_produces_a_real_incident_and_event() {
    let pg = TestPostgres::start(17).await;
    let client = pg.superuser_client().await;
    let outcome = validate(&client, Environment::Production, true)
        .await
        .expect("override lets a real superuser connection through");
    assert!(outcome.override_used);

    let repo = TestRepo::open();
    let event = detect_superuser_override(&outcome, "prod-primary", Utc::now())
        .expect("override_used must fire the detector");

    let recorded = incident::record_event(&repo.handle, event)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(security_events_for(&repo, DETECTOR_SUPERUSER_OVERRIDE), 1);
}

#[tokio::test]
async fn no_override_used_never_fires_the_detector() {
    let pg = TestPostgres::start(17).await;
    let client = pg.superuser_client().await;

    // Non-production: real superuser connection, no override needed —
    // `override_used` is genuinely false, not just unset.
    let outcome = validate(&client, Environment::Development, false)
        .await
        .expect("non-production superuser connections aren't refused");
    assert!(!outcome.override_used);

    assert!(detect_superuser_override(&outcome, "dev-box", Utc::now()).is_none());
}
