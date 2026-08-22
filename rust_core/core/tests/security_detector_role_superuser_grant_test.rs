//! Real end-to-end coverage of the superuser-grant security detector
//! (SRS FR-DBSEC-001(b), narrowed to the `rolsuper` flag, unit U56):
//! `repository::role_history::record_role_superuser_flag`'s own
//! durable diff-against-prior-snapshot persistence, `security::
//! detect_role_superuser_granted`'s pure fire/no-fire rules, and a
//! real, correlated `security_events`/`security_incidents` row —
//! mirrors `security_detector_guc_change_test.rs`'s exact shape.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{self, reader, RepositoryConfig};
use ai_ops_core::security::{
    detect_role_superuser_granted, incident, DETECTOR_ROLE_SUPERUSER_GRANTED,
};
use chrono::Utc;
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
async fn a_genuine_grant_is_known_the_second_time_even_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let first = repository::record_role_superuser_flag(&repo.handle, "app_user", false, now)
        .await
        .expect("record_role_superuser_flag");
    assert_eq!(
        first, None,
        "the very first observation must have no prior flag"
    );

    // Real second "service lifetime": a brand-new RepositoryHandle
    // against the exact same on-disk file, not the same in-process
    // handle — proves this is durable persistence.
    let db_path = repo.db_path().to_path_buf();
    let second_handle =
        repository::init(&RepositoryConfig { db_path }).expect("re-open the same database file");

    let second = repository::record_role_superuser_flag(&second_handle, "app_user", true, now)
        .await
        .expect("record_role_superuser_flag");
    assert_eq!(
        second,
        Some(false),
        "the previous flag must be durable even from a fresh connection"
    );

    let third = repository::record_role_superuser_flag(&second_handle, "app_user", true, now)
        .await
        .expect("record_role_superuser_flag");
    assert_eq!(
        third,
        Some(true),
        "an unchanged flag is still tracked as the (unchanged) prior value"
    );
}

#[test]
fn detector_fires_only_on_a_real_known_false_to_true_transition() {
    let now = Utc::now();
    assert!(detect_role_superuser_granted("app_user", Some(false), true, now).is_some());
    assert!(detect_role_superuser_granted("postgres", None, true, now).is_none());
    assert!(detect_role_superuser_granted("app_user", Some(true), true, now).is_none());
    assert!(
        detect_role_superuser_granted("app_user", Some(true), false, now).is_none(),
        "a revocation must never fire"
    );
}

#[tokio::test]
async fn a_real_superuser_grant_produces_a_real_incident_and_event() {
    let repo = TestRepo::open();
    let now = Utc::now();

    // Seed the baseline — the first observation, never a fireable event.
    let baseline = repository::record_role_superuser_flag(&repo.handle, "app_user", false, now)
        .await
        .expect("record_role_superuser_flag");
    assert!(detect_role_superuser_granted("app_user", baseline, false, now).is_none());

    // A real, genuine grant on the second poll.
    let previous = repository::record_role_superuser_flag(&repo.handle, "app_user", true, now)
        .await
        .expect("record_role_superuser_flag");
    let event = detect_role_superuser_granted("app_user", previous, true, now)
        .expect("a real grant from a known non-superuser role must fire");

    let recorded = incident::record_event(&repo.handle, event)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(
        security_events_for(&repo, DETECTOR_ROLE_SUPERUSER_GRANTED),
        1
    );
}
