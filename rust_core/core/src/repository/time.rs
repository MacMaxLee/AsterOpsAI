use chrono::{DateTime, SecondsFormat, Utc};

/// The one timestamp text format used everywhere in this module — for
/// stored `ts`/`bucket_start`/`bucket_end` columns and for query bounds
/// alike. `Z`-suffixed (not chrono's default `+00:00` offset) because
/// SQLite's `strftime`/`date`/`datetime` functions (used by the rollup
/// queries in `retention.rs`) reliably parse the `Z` form; it's also valid
/// RFC3339, so [`parse_ts`] round-trips it exactly. Millisecond precision is
/// plenty for a 1s-cadence sampler and keeps the column sortable as plain
/// text.
pub fn format_ts(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Parses a `ts` column value written by [`format_ts`]. Falls back to the
/// Unix epoch on malformed input rather than erroring — callers that care
/// about detecting corruption (the audit chain) check this explicitly
/// themselves rather than relying on a panic here.
pub fn parse_ts(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let ts = DateTime::parse_from_rfc3339("2026-01-01T12:34:56.789Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parse_ts(&format_ts(ts)), ts);
    }

    #[test]
    fn uses_z_suffix() {
        let ts = Utc::now();
        assert!(format_ts(ts).ends_with('Z'));
    }
}
