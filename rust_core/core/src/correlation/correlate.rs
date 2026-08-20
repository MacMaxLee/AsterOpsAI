//! The real cross-layer correlation (SRS FR-CORR-001): combines
//! `analysis::HostVerdict` and `analysis::DbHealthVerdict` — never raw
//! telemetry — into ranked hypotheses and an explicit ruled-out list.
//! Pure: no I/O, no AI dependency of any kind (same boundary
//! `analysis`'s own FR-PERF-005 note and `security::detector`'s own
//! FR-SEC-001 note already establish).

use chrono::{DateTime, Utc};

use crate::analysis::{
    DbHealthCategory, DbHealthCheck, DbHealthStatus, DbHealthVerdict, DomainSignal, Evidence,
    HostDomain, HostVerdict,
};

use super::cause::RootCause;
use super::confidence::{db_cause_confidence, host_cause_confidence, RULE_OUT_THRESHOLD};
use super::hypothesis::{CorrelationResult, Hypothesis, RuledOut};

struct Candidate {
    cause: RootCause,
    confidence: f64,
    evidence: Vec<Evidence>,
    reason: String,
}

fn db_checks_for<'a>(
    db: &'a DbHealthVerdict,
    categories: &[DbHealthCategory],
) -> Vec<&'a DbHealthCheck> {
    db.checks
        .iter()
        .filter(|c| categories.contains(&c.category))
        .collect()
}

fn db_reason(checks: &[&DbHealthCheck], confidence: f64) -> String {
    if checks
        .iter()
        .all(|c| c.status == DbHealthStatus::Unavailable)
    {
        "no DB evidence available".to_string()
    } else {
        format!("DB checks show no sustained problem (average severity {confidence:.2})")
    }
}

fn db_candidate(
    db: &DbHealthVerdict,
    cause: RootCause,
    categories: &[DbHealthCategory],
) -> Candidate {
    let checks = db_checks_for(db, categories);
    let confidence = db_cause_confidence(&checks);
    let evidence: Vec<Evidence> = checks.iter().flat_map(|c| c.evidence.clone()).collect();
    let reason = db_reason(&checks, confidence);
    Candidate {
        cause,
        confidence,
        evidence,
        reason,
    }
}

/// Matches the exact metric-name strings `analysis::host::classify_host`
/// tags each domain's evidence with — the only way to filter
/// `HostVerdict.evidence`'s flat list by domain, since `Evidence` itself
/// carries no domain field.
fn host_metric_name(domain: HostDomain) -> &'static str {
    match domain {
        HostDomain::Cpu => "cpu_pressure_sustained_fraction",
        HostDomain::Memory => "memory_pressure_sustained_fraction",
        HostDomain::StorageIo => "storage_io_latency_sustained_fraction",
        HostDomain::Network => "network_error_ratio_sustained_fraction",
    }
}

fn host_signal_for(host: &HostVerdict, domain: HostDomain) -> Option<&DomainSignal> {
    host.domain_signals.iter().find(|s| s.domain == domain)
}

fn host_reason(signal: Option<&DomainSignal>, confidence: f64) -> String {
    match signal {
        Some(s) if s.sample_count > 0 => {
            format!("no sustained signal (crossed fraction {confidence:.2})")
        }
        _ => "no host evidence available".to_string(),
    }
}

fn host_candidate(host: &HostVerdict, cause: RootCause, domain: HostDomain) -> Candidate {
    let signal = host_signal_for(host, domain);
    let confidence = signal.map(host_cause_confidence).unwrap_or(0.0);
    let name = host_metric_name(domain);
    let evidence: Vec<Evidence> = host
        .evidence
        .iter()
        .filter(|e| e.metric == name)
        .cloned()
        .collect();
    let reason = host_reason(signal, confidence);
    Candidate {
        cause,
        confidence,
        evidence,
        reason,
    }
}

const DB_CAUSE_MAP: [(RootCause, &[DbHealthCategory]); 4] = [
    (
        RootCause::DbLocks,
        &[
            DbHealthCategory::LockWaits,
            DbHealthCategory::Deadlocks,
            DbHealthCategory::LongTransactions,
        ],
    ),
    (
        RootCause::DbConfiguration,
        &[
            DbHealthCategory::TempFileUsage,
            DbHealthCategory::BloatProxies,
        ],
    ),
    (
        RootCause::ConnectionExhaustion,
        &[DbHealthCategory::ConnectionSaturation],
    ),
    (
        RootCause::SlowSql,
        &[DbHealthCategory::Latency, DbHealthCategory::SlowQueries],
    ),
];

const HOST_CAUSE_MAP: [(RootCause, HostDomain); 4] = [
    (RootCause::HostCpu, HostDomain::Cpu),
    (RootCause::HostMemory, HostDomain::Memory),
    (RootCause::StorageLatency, HostDomain::StorageIo),
    (RootCause::Network, HostDomain::Network),
];

/// Computed from the other eight's confidences, never its own direct
/// signal (SRS's own framing: unexplained by either host or DB
/// visibility). `m` = the strongest of the other eight; `m * (1.0 - m)`
/// peaks at a moderate, unresolved signal (`m` around `0.5`) and is `0.0`
/// at either extreme — nothing wrong anywhere (`m = 0.0`), or one cause
/// fully explains it (`m = 1.0`).
fn client_side_confidence(other_eight_max: f64) -> f64 {
    other_eight_max * (1.0 - other_eight_max)
}

pub fn correlate(
    host: &HostVerdict,
    db: &DbHealthVerdict,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> CorrelationResult {
    let mut candidates = Vec::with_capacity(RootCause::EVIDENCED.len() + 1);
    for (cause, categories) in DB_CAUSE_MAP {
        candidates.push(db_candidate(db, cause, categories));
    }
    for (cause, domain) in HOST_CAUSE_MAP {
        candidates.push(host_candidate(host, cause, domain));
    }

    let other_eight_max = candidates
        .iter()
        .map(|c| c.confidence)
        .fold(0.0_f64, f64::max);
    let confidence = client_side_confidence(other_eight_max);
    let reason = "either nothing appears to be wrong, or another cause already explains the \
                   observed signal"
        .to_string();
    candidates.push(Candidate {
        cause: RootCause::ClientSideApplication,
        confidence,
        evidence: Vec::new(),
        reason,
    });

    let mut ranked = Vec::new();
    let mut ruled_out = Vec::new();
    for c in candidates {
        if c.confidence > RULE_OUT_THRESHOLD {
            ranked.push(Hypothesis {
                cause: c.cause,
                confidence: c.confidence,
                evidence: c.evidence,
            });
        } else {
            ruled_out.push(RuledOut {
                cause: c.cause,
                reason: c.reason,
            });
        }
    }
    ranked.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));

    CorrelationResult {
        window_start,
        window_end,
        ranked,
        ruled_out,
    }
}
