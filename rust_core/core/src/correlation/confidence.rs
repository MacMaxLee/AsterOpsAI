//! The two confidence formulas (SRS FR-CORR-001's "a documented
//! formula") — deliberately different shapes for the two evidence
//! sources, not an inconsistency: DB evidence is a single-poll snapshot
//! per check (`analysis::db`'s own module doc), host evidence is a real
//! sustained window (`analysis::host::DomainSignal`). See docs/adr/0017.

use crate::analysis::{DbHealthCheck, DbHealthStatus, DomainSignal};

/// A hypothesis at or below this confidence is ruled out, not ranked —
/// same numeric-threshold judgment-call style ADR 0010/0014/0015 already
/// use for their own tier boundaries.
pub const RULE_OUT_THRESHOLD: f64 = 0.1;

fn severity(status: DbHealthStatus) -> Option<f64> {
    match status {
        DbHealthStatus::Ok => Some(0.0),
        DbHealthStatus::Warning => Some(0.5),
        DbHealthStatus::Critical => Some(1.0),
        // Excluded from both sides of the average — "no evidence" is not
        // "evidence of health."
        DbHealthStatus::Unavailable => None,
    }
}

/// The average severity of `checks`, `Unavailable` ones excluded from
/// both sides of the average; `0.0` if every one of `checks` is
/// `Unavailable` (or `checks` is empty).
pub fn db_cause_confidence(checks: &[&DbHealthCheck]) -> f64 {
    let scored: Vec<f64> = checks.iter().filter_map(|c| severity(c.status)).collect();
    if scored.is_empty() {
        return 0.0;
    }
    scored.iter().sum::<f64>() / scored.len() as f64
}

/// The domain's own sustained-signal fraction — the same
/// `crossed_count / sample_count` `DomainSignal::crossed()` already
/// computes internally, exposed here as a continuous `[0.0, 1.0]` value
/// instead of `crossed()`'s boolean.
pub fn host_cause_confidence(signal: &DomainSignal) -> f64 {
    if signal.sample_count == 0 {
        return 0.0;
    }
    signal.crossed_count as f64 / signal.sample_count as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::thresholds::Tier;
    use crate::analysis::HostDomain;

    fn check(status: DbHealthStatus) -> DbHealthCheck {
        DbHealthCheck {
            category: crate::analysis::DbHealthCategory::Latency,
            status,
            evidence: Vec::new(),
        }
    }

    #[test]
    fn db_confidence_averages_excluding_unavailable() {
        let ok = check(DbHealthStatus::Ok);
        let critical = check(DbHealthStatus::Critical);
        assert_eq!(db_cause_confidence(&[&ok, &critical]), 0.5);
    }

    #[test]
    fn db_confidence_is_zero_when_everything_is_unavailable() {
        let a = check(DbHealthStatus::Unavailable);
        let b = check(DbHealthStatus::Unavailable);
        assert_eq!(db_cause_confidence(&[&a, &b]), 0.0);
    }

    #[test]
    fn db_confidence_of_empty_slice_is_zero() {
        assert_eq!(db_cause_confidence(&[]), 0.0);
    }

    #[test]
    fn host_confidence_is_the_crossed_fraction() {
        let signal = DomainSignal {
            domain: HostDomain::Cpu,
            tier: Tier::High,
            sample_count: 4,
            crossed_count: 3,
        };
        assert_eq!(host_cause_confidence(&signal), 0.75);
    }

    #[test]
    fn host_confidence_is_zero_with_no_samples() {
        let signal = DomainSignal {
            domain: HostDomain::Cpu,
            tier: Tier::Normal,
            sample_count: 0,
            crossed_count: 0,
        };
        assert_eq!(host_cause_confidence(&signal), 0.0);
    }
}
