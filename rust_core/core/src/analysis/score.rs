//! Two frozen, versioned weight tables (SRS FR-PERF-004): introducing a new
//! weight set adds a new `*_SCORE_VERSION` constant, it never mutates an
//! existing one in place, and historical rows are never re-scored. Both
//! functions are pure arithmetic over already-computed evidence — no I/O,
//! no AI (FR-PERF-005).

use super::db::{DbHealthCategory, DbHealthCheck, DbHealthStatus};
use super::host::DomainSignal;
use super::thresholds::Tier;

pub const HOST_SCORE_VERSION: &str = "host-v1";
pub const DB_SCORE_VERSION: &str = "db-v1";

const HOST_TIER_PENALTY_ELEVATED: u32 = 5;
const HOST_TIER_PENALTY_HIGH: u32 = 15;
const HOST_TIER_PENALTY_CRITICAL: u32 = 30;

fn host_domain_penalty(tier: Tier) -> u32 {
    match tier {
        Tier::Normal => 0,
        Tier::Elevated => HOST_TIER_PENALTY_ELEVATED,
        Tier::High => HOST_TIER_PENALTY_HIGH,
        Tier::Critical => HOST_TIER_PENALTY_CRITICAL,
    }
}

/// 100 minus a fixed per-domain-tier penalty, summed across every domain
/// that had data (a domain with zero samples contributes no penalty —
/// missing data is never treated as "bad"), floored at 0.
pub fn score_host(domain_signals: &[DomainSignal]) -> u8 {
    let total_penalty: u32 = domain_signals
        .iter()
        .filter(|s| s.sample_count > 0)
        .map(|s| host_domain_penalty(s.tier))
        .sum();
    100u32.saturating_sub(total_penalty).min(100) as u8
}

const DB_CHECK_PENALTY_WARNING: u32 = 10;
const DB_CHECK_PENALTY_CRITICAL: u32 = 25;

fn db_check_penalty(status: DbHealthStatus) -> u32 {
    match status {
        DbHealthStatus::Ok | DbHealthStatus::Unavailable => 0,
        DbHealthStatus::Warning => DB_CHECK_PENALTY_WARNING,
        DbHealthStatus::Critical => DB_CHECK_PENALTY_CRITICAL,
    }
}

/// If `Availability` itself is `Critical` (the poll that would feed this
/// verdict failed outright — see `db::unavailable_verdict`), the score is 0
/// unconditionally: every other category is `Unavailable` in that case
/// anyway, so summing their (zero) penalties would misleadingly imply a
/// clean bill of health. Otherwise: 100 minus a fixed per-category-status
/// penalty summed across every non-`Availability` category, floored at 0.
pub fn score_db(checks: &[DbHealthCheck]) -> u8 {
    let availability_critical = checks.iter().any(|c| {
        c.category == DbHealthCategory::Availability && c.status == DbHealthStatus::Critical
    });
    if availability_critical {
        return 0;
    }
    let total_penalty: u32 = checks
        .iter()
        .filter(|c| c.category != DbHealthCategory::Availability)
        .map(|c| db_check_penalty(c.status))
        .sum();
    100u32.saturating_sub(total_penalty).min(100) as u8
}
