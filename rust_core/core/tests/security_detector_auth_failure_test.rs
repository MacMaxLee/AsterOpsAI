//! Real end-to-end coverage of the authentication-failure security
//! detector (SRS FR-DBSEC-001(a), the last FR-DBSEC-001 sub-item,
//! unit U60 — see docs/adr/0065 for the full scope decision): a real
//! CSV log fixture file on disk, `dbms::log_tail::read_new_auth_
//! failures`'s own real file-reading + `sql_state_code` filtering,
//! `security::detect_auth_failure`'s pure (always-fires) construction,
//! and a real, correlated `security_events`/`security_incidents` row.
//! The real *production* CSV format (from an actual running
//! PostgreSQL instance) is proven separately, over real HTTP, in
//! `service/tests/dbms_endpoints.rs`.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::dbms::log_tail::read_new_auth_failures;
use ai_ops_core::repository::reader;
use ai_ops_core::security::{detect_auth_failure, incident, DETECTOR_AUTH_FAILURE};
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

/// Builds one properly CSV-encoded row from raw field values — using
/// the same `csv` crate this test's own production code
/// (`dbms::log_tail`) parses with, rather than hand-escaping quotes,
/// the same "don't hand-roll something a real parser already gets
/// right" reasoning documented in this unit's own ADR.
fn csv_row(fields: &[&str]) -> String {
    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(vec![]);
    wtr.write_record(fields).expect("write_record");
    String::from_utf8(wtr.into_inner().expect("into_inner")).expect("utf8")
}

/// Real PostgreSQL `csvlog` rows (26 columns, confirmed empirically
/// against a real PG17 instance during this unit's own research) —
/// one ordinary `LOG`-level connection line that must be filtered
/// out, one real `FATAL`/`28P01` auth-failure line that must not be.
fn fixture_csv_content() -> String {
    let log_row = csv_row(&[
        "2026-08-19 12:00:00.000 UTC",
        "",
        "",
        "12345",
        "127.0.0.1:40000",
        "abc.1",
        "1",
        "",
        "2026-08-19 12:00:00 UTC",
        "",
        "0",
        "LOG",
        "00000",
        "connection received: host=127.0.0.1 port=40000",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "client backend",
        "",
        "0",
    ]);
    let fatal_row = csv_row(&[
        "2026-08-19 12:00:01.000 UTC",
        "pwuser",
        "postgres",
        "12346",
        "127.0.0.1:46060",
        "abc.2",
        "1",
        "",
        "2026-08-19 12:00:01 UTC",
        "",
        "0",
        "FATAL",
        "28P01",
        "password authentication failed for user \"pwuser\"",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "client backend",
        "",
        "0",
    ]);
    format!("{log_row}{fatal_row}")
}

#[tokio::test]
async fn tailing_a_real_csv_log_file_finds_only_the_real_auth_failure_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("postgresql-2026-08-19_120000.csv");
    std::fs::write(&log_path, fixture_csv_content()).expect("write fixture");

    let (events, tailed_file, offset) = read_new_auth_failures(dir.path(), None)
        .await
        .expect("read_new_auth_failures");

    assert_eq!(tailed_file, log_path);
    assert!(offset > 0, "a non-empty file must advance the offset");
    assert_eq!(
        events.len(),
        1,
        "the LOG-level connection row must be filtered out"
    );
    assert_eq!(events[0].user_name.as_deref(), Some("pwuser"));
    assert_eq!(
        events[0].connection_from.as_deref(),
        Some("127.0.0.1:46060")
    );
    assert!(events[0].message.contains("password authentication failed"));
}

#[tokio::test]
async fn re_tailing_from_the_persisted_offset_returns_no_events_even_though_the_file_still_exists()
{
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("postgresql-2026-08-19_120000.csv");
    std::fs::write(&log_path, fixture_csv_content()).expect("write fixture");

    let (_events, tailed_file, offset) = read_new_auth_failures(dir.path(), None)
        .await
        .expect("first read_new_auth_failures");

    let (events_again, _tailed_file, offset_again) =
        read_new_auth_failures(dir.path(), Some((tailed_file, offset)))
            .await
            .expect("second read_new_auth_failures");

    assert!(
        events_again.is_empty(),
        "re-tailing from the already-consumed offset must find nothing new"
    );
    assert_eq!(
        offset_again, offset,
        "the offset must not advance when there's nothing new to read"
    );
}

#[test]
fn detector_always_produces_an_event_never_none() {
    // No fire/no-fire branch to test here beyond "it always constructs
    // something" — `detect_auth_failure` isn't `Option`-returning, so
    // this exercises the pure construction path directly, unlike every
    // other detector's own `_fires_only_when_` test.
    let now = Utc::now();
    let event = ai_ops_core::dbms::log_tail::AuthFailureEvent {
        user_name: Some("pwuser".to_string()),
        database_name: Some("postgres".to_string()),
        connection_from: Some("127.0.0.1:46060".to_string()),
        message: "password authentication failed for user \"pwuser\"".to_string(),
        log_time: now,
    };
    let detected = detect_auth_failure(&event, now);
    assert_eq!(detected.detector_id, DETECTOR_AUTH_FAILURE);
}

#[tokio::test]
async fn a_real_tailed_auth_failure_produces_a_real_incident_and_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("postgresql-2026-08-19_120000.csv");
    std::fs::write(&log_path, fixture_csv_content()).expect("write fixture");

    let (events, _tailed_file, _offset) = read_new_auth_failures(dir.path(), None)
        .await
        .expect("read_new_auth_failures");
    assert_eq!(events.len(), 1);

    let repo = TestRepo::open();
    let now = Utc::now();
    let detected = detect_auth_failure(&events[0], now);

    let recorded = incident::record_event(&repo.handle, detected)
        .await
        .expect("record_event");
    match recorded {
        incident::IncidentOutcome::Recorded { .. } => {}
        other => panic!("expected Recorded, got {other:?}"),
    }
    assert_eq!(security_events_for(&repo, DETECTOR_AUTH_FAILURE), 1);
}
