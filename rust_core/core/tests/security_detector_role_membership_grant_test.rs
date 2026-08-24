//! Real end-to-end coverage of the role-membership-grant security
//! detector (SRS FR-DBSEC-001(b)'s own deferred remainder, unit U58):
//! `repository::role_membership_history::record_seen`'s own durable
//! "have we ever seen this pair before" persistence, `security::
//! detect_role_membership_granted`'s pure fire/no-fire rules
//! (including the `pg_*` system-role filter), and a real, correlated
//! `security_events`/`security_incidents` row — mirrors
//! `security_detector_unusual_client_address_test.rs`'s exact shape.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{self, reader, RepositoryConfig};
use ai_ops_core::security::{
    detect_role_membership_granted, incident, DETECTOR_ROLE_MEMBERSHIP_GRANTED,
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
async fn a_pair_seen_twice_is_known_the_second_time_even_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let first = repository::record_role_membership_seen(&repo.handle, "alice", "admins", now)
        .await
        .expect("record_role_membership_seen");
    assert!(
        !first,
        "the very first observation must not be 'already known'"
    );

    // Real second "service lifetime": a brand-new RepositoryHandle
    // against the exact same on-disk file, not the same in-process
    // handle — proves this is durable persistence.
    let db_path = repo.db_path().to_path_buf();
    let second_handle =
        repository::init(&RepositoryConfig { db_path }).expect("re-open the same database file");

    let second = repository::record_role_membership_seen(&second_handle, "alice", "admins", now)
        .await
        .expect("record_role_membership_seen");
    assert!(
        second,
        "a pair already recorded must be 'already known' even from a fresh connection"
    );

    let third = repository::record_role_membership_seen(&second_handle, "alice", "readonly", now)
        .await
        .expect("record_role_membership_seen");
    assert!(
        !third,
        "the same member with a genuinely different granted_role is not known"
    );
}

#[tokio::test]
async fn a_revoked_pair_is_forgotten_and_a_genuine_re_grant_fires_again() {
    let repo = TestRepo::open();
    let now = Utc::now();

    // alice is granted admins: first observation, not yet known.
    let first = repository::record_role_membership_seen(&repo.handle, "alice", "admins", now)
        .await
        .expect("record_role_membership_seen");
    assert!(!first, "the first observation must not be 'already known'");

    // Next sweep tick: alice is no longer a member of admins (revoked)
    // — the current set no longer contains the pair.
    let forgotten = repository::reconcile_known_role_memberships(&repo.handle, vec![])
        .await
        .expect("reconcile_known_role_memberships");
    assert_eq!(forgotten, 1, "the revoked pair must be forgotten");

    // alice is re-granted admins later: without reconciliation this
    // would still read as 'already known' and silently never fire.
    let after_regrant =
        repository::record_role_membership_seen(&repo.handle, "alice", "admins", now)
            .await
            .expect("record_role_membership_seen");
    assert!(
        !after_regrant,
        "a genuine re-grant after a real revocation must not read as 'already known'"
    );
}

#[tokio::test]
async fn reconcile_leaves_pairs_still_present_in_the_current_set_alone() {
    let repo = TestRepo::open();
    let now = Utc::now();

    repository::record_role_membership_seen(&repo.handle, "alice", "admins", now)
        .await
        .expect("record_role_membership_seen");

    let forgotten = repository::reconcile_known_role_memberships(
        &repo.handle,
        vec![("alice".to_string(), "admins".to_string())],
    )
    .await
    .expect("reconcile_known_role_memberships");
    assert_eq!(
        forgotten, 0,
        "a pair still present in the current set must not be forgotten"
    );

    let still_known = repository::record_role_membership_seen(&repo.handle, "alice", "admins", now)
        .await
        .expect("record_role_membership_seen");
    assert!(
        still_known,
        "an unrevoked pair must remain known after reconciliation"
    );
}

#[test]
fn detector_fires_only_for_a_new_non_system_pair() {
    let now = Utc::now();
    assert!(detect_role_membership_granted("alice", "admins", false, now).is_some());
    assert!(detect_role_membership_granted("alice", "admins", true, now).is_none());
    assert!(
        detect_role_membership_granted("pg_monitor", "pg_read_all_stats", false, now).is_none(),
        "a built-in pg_* system role member must never fire"
    );
}

#[tokio::test]
async fn a_real_role_membership_grant_produces_a_real_incident_and_event() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let already_known =
        repository::record_role_membership_seen(&repo.handle, "alice", "admins", now)
            .await
            .expect("record_role_membership_seen");
    let event = detect_role_membership_granted("alice", "admins", already_known, now)
        .expect("a genuinely new, non-system pair must fire");

    let recorded = incident::record_event(&repo.handle, event)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(
        security_events_for(&repo, DETECTOR_ROLE_MEMBERSHIP_GRANTED),
        1
    );
}
