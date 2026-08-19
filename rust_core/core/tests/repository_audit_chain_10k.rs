//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod common;

use ai_ops_core::repository::{self, audit, NewAuditEvent};
use chrono::Utc;
use common::TestRepo;

const CHAIN_LEN: usize = 10_000;

#[tokio::test]
async fn ten_thousand_row_chain_verifies_clean() {
    let repo = TestRepo::open();

    for i in 0..CHAIN_LEN {
        let new = NewAuditEvent {
            ts: Utc::now(),
            event_type: "TEST_EVENT".to_string(),
            actor: "test-harness".to_string(),
            summary: format!("event {i}"),
            detail_json: format!("{{\"i\":{i}}}"),
        };
        repository::record_audit_event(&repo.handle, new)
            .await
            .unwrap();
    }

    let conn = repository::reader::checkout(&repo.handle.read_pool).unwrap();
    let verification = audit::verify_chain(&conn).unwrap();
    assert_eq!(verification.total_rows, CHAIN_LEN as u64);
    assert_eq!(verification.first_broken_id, None);
}

#[tokio::test]
async fn a_tampered_row_is_detected_at_the_right_id() {
    let repo = TestRepo::open();

    for i in 0..CHAIN_LEN {
        let new = NewAuditEvent {
            ts: Utc::now(),
            event_type: "TEST_EVENT".to_string(),
            actor: "test-harness".to_string(),
            summary: format!("event {i}"),
            detail_json: format!("{{\"i\":{i}}}"),
        };
        repository::record_audit_event(&repo.handle, new)
            .await
            .unwrap();
    }

    // Simulate "a manually edited row": bypass the repository layer
    // entirely and mutate the file directly with a raw connection, exactly
    // as a hex-editor or a direct `sqlite3` CLI session would.
    repository::shutdown(&repo.handle).await.unwrap();
    let db_path = repo.db_path();
    let raw = rusqlite::Connection::open(db_path).unwrap();
    raw.execute(
        "UPDATE audit_events SET summary = 'tampered' WHERE id = ?1",
        [5000_i64],
    )
    .unwrap();
    drop(raw);

    let verify_conn = rusqlite::Connection::open(db_path).unwrap();
    let verification = audit::verify_chain(&verify_conn).unwrap();
    assert_eq!(verification.total_rows, CHAIN_LEN as u64);
    assert_eq!(verification.first_broken_id, Some(5000));
}
