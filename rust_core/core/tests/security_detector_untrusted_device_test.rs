//! Real end-to-end coverage of `repository::device_trust::record_seen`
//! (unit U11): proves "known" is genuinely durable across two independent
//! calls against the same on-disk SQLite file — the real gap this unit
//! closes (the service sampler's own prior tracking was an ephemeral,
//! in-process `HashSet` that reset on every restart) — then that
//! `security::detect_untrusted_device` only fires for a new, removable
//! device — SRS FR-DEV-001's own "attached storage/removable/peripheral
//! devices with a stable identifier" requirement, real and durable.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod repo_common;

use ai_ops_core::repository::{self, RepositoryConfig};
use ai_ops_core::security::detect_untrusted_device;
use chrono::Utc;
use repo_common::TestRepo;

#[tokio::test]
async fn a_device_seen_twice_is_known_the_second_time_even_across_a_fresh_connection() {
    let repo = TestRepo::open();
    let now = Utc::now();

    let first = repository::record_device_seen(&repo.handle, "block:sdb", now)
        .await
        .expect("record_device_seen");
    assert!(
        !first,
        "the very first observation must not be 'already known'"
    );

    // Real second "service lifetime": a brand-new RepositoryHandle against
    // the exact same on-disk file, not the same in-process handle — proves
    // this is durable persistence, not merely a cached value on `repo`.
    let db_path = repo.db_path().to_path_buf();
    let second_handle =
        repository::init(&RepositoryConfig { db_path }).expect("re-open the same database file");

    let second = repository::record_device_seen(&second_handle, "block:sdb", now)
        .await
        .expect("record_device_seen");
    assert!(
        second,
        "a device already recorded must be 'already known' even from a fresh connection"
    );

    let third = repository::record_device_seen(&second_handle, "block:sdc", now)
        .await
        .expect("record_device_seen");
    assert!(!third, "a genuinely different identifier is not known");
}

#[test]
fn detector_fires_only_for_a_new_removable_device() {
    let now = Utc::now();
    assert!(detect_untrusted_device("block:sdb", "sdb", true, false, now).is_some());
    assert!(detect_untrusted_device("block:sdb", "sdb", true, true, now).is_none());
    assert!(detect_untrusted_device("block:sda", "sda", false, false, now).is_none());
}
