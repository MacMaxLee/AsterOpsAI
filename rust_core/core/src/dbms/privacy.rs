//! SQL privacy (TRS §19, SRS NFR-PRIV-001): default capture mode is
//! normalized query text in every environment; raw text is an explicit
//! per-connection opt-in (`ConnectionMetadata::capture_raw_sql`).
//!
//! `pg_stat_statements.query` is *always* pre-normalized by PostgreSQL
//! itself — there is no raw form available from that source at all, so
//! `query_stats()` needs no redaction/opt-in logic of its own (see
//! `QueryStat`'s doc comment in `mod.rs`). The actual raw-SQL source in
//! this codebase is `pg_stat_activity.query` (a session's current/last
//! query, verbatim as sent) — surfaced through `SessionInfo.query` and
//! `LongTransaction.query`, both routed through `sanitize_query` here
//! before ever leaving the adapter.

use std::sync::LazyLock;

use regex::Regex;

/// Anything shaped like `password=...`, `secret=...`, `pwd=...`, or a
/// PostgreSQL connection string containing one of those, redacted
/// regardless of the `capture_raw_sql` opt-in — this is a hard
/// requirement (requirement 7: "strip ... before it can reach any AI
/// prompt"), not something the opt-in can waive.
static CREDENTIAL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    // a fixed, hand-verified pattern; a bad regex here is a compile-time-obvious bug, not a runtime one
    Regex::new(r"(?i)(password|passwd|pwd|secret|api[_-]?key)\s*[=:]\s*\S+").unwrap()
});

/// A single-quoted string literal, `'...'`, with `''`-escaped quotes
/// handled — replaced with `?` during normalization.
static STRING_LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"'(?:[^']|'')*'").unwrap()
});

/// A bare numeric literal (not part of an identifier) — replaced with `?`
/// during normalization.
static NUMERIC_LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"\b\d+(\.\d+)?\b").unwrap()
});

fn redact_credential_patterns(text: &str) -> String {
    CREDENTIAL_PATTERN
        .replace_all(text, "$1=<redacted>")
        .into_owned()
}

/// Crude but real literal-substitution normalization for query text that
/// doesn't come pre-normalized from `pg_stat_statements` — replaces
/// string and numeric literals with `?`, approximating (not replicating
/// exactly) `pg_stat_statements`' own normalization algorithm for this
/// different data source.
fn normalize(text: &str) -> String {
    let no_strings = STRING_LITERAL.replace_all(text, "?");
    NUMERIC_LITERAL.replace_all(&no_strings, "?").into_owned()
}

/// The one place `SessionInfo.query`/`LongTransaction.query` get built —
/// always redacts credential-shaped substrings first, then normalizes
/// unless `capture_raw_sql` is set.
pub fn sanitize_query(raw: Option<&str>, capture_raw_sql: bool) -> Option<String> {
    let raw = raw?;
    if raw.is_empty() {
        return None;
    }
    let redacted = redact_credential_patterns(raw);
    Some(if capture_raw_sql {
        redacted
    } else {
        normalize(&redacted)
    })
}

// Unit tests for this module live in core/tests/dbms/privacy_test.rs, not
// inline here: their fixtures are necessarily SQL-*shaped* strings (that's
// the whole point — this module normalizes query text), which the
// check-no-sql-outside-adapters.sh grep gate can't distinguish from a real
// executed query. core/tests/ is the gate's own established exception for
// exactly this class of case (see U2's audit-chain tamper test and its own
// raw-SQL assertions), so the tests live there instead of earning a gate
// false-positive here.
