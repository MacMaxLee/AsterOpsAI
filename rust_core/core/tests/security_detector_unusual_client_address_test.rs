//! Real end-to-end coverage of the unusual-client-address security
//! detector (SRS FR-DBSEC-001(d), unit U57): `repository::client_
//! address_history::record_seen`'s own durable "have we ever seen
//! this address before" persistence, `security::detect_unusual_
//! client_address`'s pure fire/no-fire rules, and a real, correlated
//! `security_events`/`security_incidents` row — mirrors
//! `security_detector_untrusted_device_test.rs`'s exact shape (the
//! same boolean "already known" detector family, not the diff-based
//! GUC/role-grant shape).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{self, reader, RepositoryConfig};
use ai_ops_core::security::{
    detect_unusual_client_address, incident, DETECTOR_UNUSUAL_CLIENT_ADDRESS,
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
async fn an_address_seen_twice_is_known_the_second_time_even_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let first = repository::record_client_address_seen(&repo.handle, "203.0.113.7", now)
        .await
        .expect("record_client_address_seen");
    assert!(
        !first,
        "the very first observation must not be 'already known'"
    );

    // Real second "service lifetime": a brand-new RepositoryHandle
    // against the exact same on-disk file, not the same in-process
    // handle — proves this is durable persistence, matching
    // `record_device_seen`'s own equivalent proof.
    let db_path = repo.db_path().to_path_buf();
    let second_handle =
        repository::init(&RepositoryConfig { db_path }).expect("re-open the same database file");

    let second = repository::record_client_address_seen(&second_handle, "203.0.113.7", now)
        .await
        .expect("record_client_address_seen");
    assert!(
        second,
        "an address already recorded must be 'already known' even from a fresh connection"
    );

    let third = repository::record_client_address_seen(&second_handle, "198.51.100.9", now)
        .await
        .expect("record_client_address_seen");
    assert!(!third, "a genuinely different address is not known");
}

#[test]
fn detector_fires_only_for_a_genuinely_new_address() {
    let now = Utc::now();
    assert!(detect_unusual_client_address("203.0.113.7", false, now).is_some());
    assert!(detect_unusual_client_address("203.0.113.7", true, now).is_none());
}

#[tokio::test]
async fn a_real_unusual_address_produces_a_real_incident_and_event() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let already_known = repository::record_client_address_seen(&repo.handle, "203.0.113.7", now)
        .await
        .expect("record_client_address_seen");
    let event = detect_unusual_client_address("203.0.113.7", already_known, now)
        .expect("a genuinely new address must fire");

    let recorded = incident::record_event(&repo.handle, event)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(
        security_events_for(&repo, DETECTOR_UNUSUAL_CLIENT_ADDRESS),
        1
    );
}
