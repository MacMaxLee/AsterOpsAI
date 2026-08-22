//! Real end-to-end coverage of the table permission-grant security
//! detector (SRS FR-DBSEC-001(c), deliberately narrowed, unit U59 —
//! see docs/adr/0064 for the full scope decision): `repository::
//! table_privilege_history::record_seen`'s own durable "have we ever
//! seen this tuple before" persistence, `security::detect_table_
//! privilege_granted`'s pure fire/no-fire rules, and a real,
//! correlated `security_events`/`security_incidents` row — mirrors
//! `security_detector_role_membership_grant_test.rs`'s exact shape.
//! The real SQL query's own filtering (system schemas, owner-implicit
//! grants) is proven separately, over real HTTP against a real
//! PostgreSQL instance, in `service/tests/dbms_endpoints.rs`.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{self, reader, RepositoryConfig};
use ai_ops_core::security::{
    detect_table_privilege_granted, incident, DETECTOR_TABLE_PRIVILEGE_GRANTED,
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
async fn a_tuple_seen_twice_is_known_the_second_time_even_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let first = repository::record_table_privilege_grant_seen(
        &repo.handle,
        "alice",
        "public",
        "widgets",
        "SELECT",
        now,
    )
    .await
    .expect("record_table_privilege_grant_seen");
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

    let second = repository::record_table_privilege_grant_seen(
        &second_handle,
        "alice",
        "public",
        "widgets",
        "SELECT",
        now,
    )
    .await
    .expect("record_table_privilege_grant_seen");
    assert!(
        second,
        "a tuple already recorded must be 'already known' even from a fresh connection"
    );

    let third = repository::record_table_privilege_grant_seen(
        &second_handle,
        "alice",
        "public",
        "widgets",
        "INSERT",
        now,
    )
    .await
    .expect("record_table_privilege_grant_seen");
    assert!(
        !third,
        "the same (grantee, schema, table) with a genuinely different \
         privilege_type is not known"
    );
}

#[test]
fn detector_fires_only_for_a_new_tuple() {
    let now = Utc::now();
    assert!(
        detect_table_privilege_granted("alice", "public", "widgets", "SELECT", false, now)
            .is_some()
    );
    assert!(
        detect_table_privilege_granted("alice", "public", "widgets", "SELECT", true, now).is_none()
    );
}

#[tokio::test]
async fn a_real_table_privilege_grant_produces_a_real_incident_and_event() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let already_known = repository::record_table_privilege_grant_seen(
        &repo.handle,
        "alice",
        "public",
        "widgets",
        "SELECT",
        now,
    )
    .await
    .expect("record_table_privilege_grant_seen");
    let event =
        detect_table_privilege_granted("alice", "public", "widgets", "SELECT", already_known, now)
            .expect("a genuinely new tuple must fire");

    let recorded = incident::record_event(&repo.handle, event)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(
        security_events_for(&repo, DETECTOR_TABLE_PRIVILEGE_GRANTED),
        1
    );
}
