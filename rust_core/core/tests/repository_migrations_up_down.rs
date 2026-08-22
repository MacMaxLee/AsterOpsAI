//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod common;

use ai_ops_core::repository::{self, RepositoryConfig};
use common::TestRepo;
use rusqlite::Connection;

const EXPECTED_TABLES: &[&str] = &[
    "telemetry_snapshots",
    "telemetry_rollup_hourly",
    "telemetry_rollup_daily",
    "audit_events",
    "actions",
    "performance_analysis",
    "security_incidents",
    "security_events",
    "dbms_connections",
    "dbms_snapshots",
    "benchmark_runs",
    "tuning_plans",
    "known_devices",
    "security_suppressions",
    "known_guc_values",
    "known_role_superuser_flags",
    "known_client_addresses",
    "known_role_memberships",
    "known_table_privilege_grants",
];

fn table_names(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != 'refinery_schema_history'")
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn migrations_create_every_expected_table() {
    let repo = TestRepo::open();
    let conn = repository::reader::checkout(&repo.handle.read_pool).unwrap();
    let names = table_names(&conn);
    for expected in EXPECTED_TABLES {
        assert!(
            names.iter().any(|n| n == expected),
            "expected table {expected} to exist, found {names:?}"
        );
    }
    assert!(
        !names.iter().any(|n| n == "dbms_metrics"),
        "V8 drops dbms_metrics (superseded by dbms_connections/dbms_snapshots) \
         but it's still present: {names:?}"
    );
}

#[test]
fn a_second_init_on_an_already_migrated_db_is_a_safe_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.sqlite3");

    let first = repository::init(&RepositoryConfig {
        db_path: db_path.clone(),
    })
    .unwrap();
    let conn = repository::reader::checkout(&first.read_pool).unwrap();
    let names_after_first = table_names(&conn);
    drop(conn);

    // Open a second time against the same (already-migrated) file — this
    // must succeed and change nothing, per requirement 3's "refuse to
    // start on failure, never touch existing data" guarantee applying
    // equally to the ordinary "already up to date" case.
    let second = repository::init(&RepositoryConfig { db_path }).unwrap();
    let conn2 = repository::reader::checkout(&second.read_pool).unwrap();
    let names_after_second = table_names(&conn2);

    assert_eq!(names_after_first, names_after_second);
}
