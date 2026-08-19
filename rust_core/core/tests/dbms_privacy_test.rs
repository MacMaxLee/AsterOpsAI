//! `dbms::privacy::sanitize_query` (TRS §19 / SRS NFR-PRIV-001). Lives here,
//! not as an inline `#[cfg(test)]` module in `privacy.rs`, because its
//! fixtures are necessarily SQL-*shaped* strings — exactly what
//! `scripts/check-no-sql-outside-adapters.sh` can't tell apart from a real
//! executed query. `core/tests/` is the gate's own established exception
//! for this (see repository_audit_chain_10k.rs's raw tamper-test SQL).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::dbms::privacy::sanitize_query;

#[test]
fn normalizes_by_default() {
    let sanitized = sanitize_query(Some("SELECT * FROM users WHERE id = 42"), false);
    assert_eq!(
        sanitized.as_deref(),
        Some("SELECT * FROM users WHERE id = ?")
    );
}

#[test]
fn keeps_raw_text_when_opted_in() {
    let sanitized = sanitize_query(Some("SELECT * FROM users WHERE id = 42"), true);
    assert_eq!(
        sanitized.as_deref(),
        Some("SELECT * FROM users WHERE id = 42")
    );
}

#[test]
fn redacts_credential_patterns_even_when_raw_capture_is_on() {
    let sanitized = sanitize_query(
        Some("SELECT dblink_connect('host=x password=hunter2')"),
        true,
    );
    let text = sanitized.unwrap();
    assert!(!text.contains("hunter2"));
    assert!(text.contains("<redacted>"));
}

#[test]
fn none_stays_none() {
    assert_eq!(sanitize_query(None, false), None);
    assert_eq!(sanitize_query(Some(""), false), None);
}

#[test]
fn normalizes_string_literals_too() {
    let sanitized = sanitize_query(Some("SELECT * FROM t WHERE name = 'alice'"), false);
    assert_eq!(sanitized.as_deref(), Some("SELECT * FROM t WHERE name = ?"));
}
