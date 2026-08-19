use chrono::{DateTime, Utc};
use std::time::Duration;

/// Everything a parser needs to know about *when* a sample was taken, always
/// supplied by the caller — never read internally via `Utc::now()`/
/// `Instant::now()`. This is what makes insta snapshots deterministic and
/// lets tests exercise `SampleGap` by simply passing a large `elapsed`
/// against unchanged fixture content (the "suspend-gap" scenario).
#[derive(Debug, Clone, Copy)]
pub struct SampleContext {
    pub now: DateTime<Utc>,
    pub elapsed: Duration,
    pub configured_interval: Duration,
}
