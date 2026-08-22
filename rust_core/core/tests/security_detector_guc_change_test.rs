//! Real end-to-end coverage of the configuration-change security
//! detector (SRS FR-DBSEC-001(e), unit U55): `repository::guc_history::
//! record_guc_value`'s own durable diff-against-prior-snapshot
//! persistence, `security::detect_guc_change`'s pure fire/no-fire
//! rules, and a real, correlated `security_events`/`security_incidents`
//! row produced by chaining both against a real SQLite-backed
//! repository — the same shape `security_detector_untrusted_device_
//! test.rs`/`security_detector_superuser_override_test.rs` already
//! established for the other two detectors.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{self, reader, RepositoryConfig};
use ai_ops_core::security::{detect_guc_change, incident, DETECTOR_GUC_CHANGED};
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
async fn a_genuinely_changed_setting_is_known_the_second_time_even_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let first = repository::record_guc_value(&repo.handle, "autovacuum", "on", now)
        .await
        .expect("record_guc_value");
    assert_eq!(
        first, None,
        "the very first observation must have no prior value"
    );

    // Real second "service lifetime": a brand-new RepositoryHandle
    // against the exact same on-disk file, not the same in-process
    // handle — proves this is durable persistence, matching
    // `record_device_seen`'s own equivalent proof.
    let db_path = repo.db_path().to_path_buf();
    let second_handle =
        repository::init(&RepositoryConfig { db_path }).expect("re-open the same database file");

    let second = repository::record_guc_value(&second_handle, "autovacuum", "off", now)
        .await
        .expect("record_guc_value");
    assert_eq!(
        second,
        Some("on".to_string()),
        "the previous setting must be durable even from a fresh connection"
    );

    let third = repository::record_guc_value(&second_handle, "autovacuum", "off", now)
        .await
        .expect("record_guc_value");
    assert_eq!(
        third,
        Some("off".to_string()),
        "an unchanged value is still tracked as the (unchanged) prior setting"
    );
}

#[test]
fn detector_fires_only_for_a_real_change_from_a_known_prior_value() {
    let now = Utc::now();
    assert!(detect_guc_change("autovacuum", Some("on"), "off", now).is_some());
    assert!(detect_guc_change("autovacuum", None, "on", now).is_none());
    assert!(detect_guc_change("autovacuum", Some("on"), "on", now).is_none());
}

#[tokio::test]
async fn a_real_guc_change_produces_a_real_incident_and_event() {
    let repo = TestRepo::open();
    let now = Utc::now();

    // Seed the baseline — the first observation, never a fireable event.
    let baseline = repository::record_guc_value(&repo.handle, "autovacuum", "on", now)
        .await
        .expect("record_guc_value");
    assert!(detect_guc_change("autovacuum", baseline.as_deref(), "on", now).is_none());

    // A real, genuinely changed second poll.
    let previous = repository::record_guc_value(&repo.handle, "autovacuum", "off", now)
        .await
        .expect("record_guc_value");
    let event = detect_guc_change("autovacuum", previous.as_deref(), "off", now)
        .expect("a real change from a known prior value must fire");

    let recorded = incident::record_event(&repo.handle, event)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(security_events_for(&repo, DETECTOR_GUC_CHANGED), 1);
}
