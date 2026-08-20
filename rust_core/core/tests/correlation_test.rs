//! Real coverage of `correlation::correlate` (SRS FR-CORR-001..002, unit
//! U12): hand-built `HostVerdict`/`DbHealthVerdict` fixtures (every field
//! is `pub` — the same real production output types `analysis::
//! classify_host`/`classify_db` themselves produce, constructed directly
//! here to test `correlate`'s own logic in isolation from their
//! threshold classification, which `analysis_host_classification_test.rs`/
//! `analysis_db_health_*_test.rs` already cover on their own).
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::analysis::{
    self, DbHealthCategory, DbHealthCheck, DbHealthStatus, DbHealthVerdict, DomainSignal, Evidence,
    HostBottleneck, HostDomain, HostVerdict,
};
use ai_ops_core::correlation::{correlate, RootCause};
use chrono::Utc;

fn ok_check(category: DbHealthCategory) -> DbHealthCheck {
    DbHealthCheck {
        category,
        status: DbHealthStatus::Ok,
        evidence: Vec::new(),
    }
}

/// A check at the given non-`Ok`, non-`Unavailable` status, with one
/// real evidence entry — `Unavailable` checks carry no evidence, same as
/// `analysis::db`'s own `unavailable_check` never does.
fn status_check(category: DbHealthCategory, status: DbHealthStatus) -> DbHealthCheck {
    if status == DbHealthStatus::Unavailable {
        return DbHealthCheck {
            category,
            status,
            evidence: Vec::new(),
        };
    }
    DbHealthCheck {
        category,
        status,
        evidence: vec![Evidence::new(
            "test_metric",
            1.0,
            0.5,
            None,
            Utc::now(),
            Utc::now(),
        )],
    }
}

const ALL_DB_CATEGORIES: [DbHealthCategory; 11] = [
    DbHealthCategory::Availability,
    DbHealthCategory::ConnectionSaturation,
    DbHealthCategory::Latency,
    DbHealthCategory::SlowQueries,
    DbHealthCategory::LockWaits,
    DbHealthCategory::Deadlocks,
    DbHealthCategory::RollbackRatio,
    DbHealthCategory::TempFileUsage,
    DbHealthCategory::LongTransactions,
    DbHealthCategory::ReplicationLag,
    DbHealthCategory::BloatProxies,
];

/// A DB verdict where every category is `Ok` except the ones in
/// `overrides`.
fn db_verdict(overrides: &[(DbHealthCategory, DbHealthStatus)]) -> DbHealthVerdict {
    let checks = ALL_DB_CATEGORIES
        .into_iter()
        .map(
            |category| match overrides.iter().find(|(c, _)| *c == category) {
                Some((_, status)) => status_check(category, *status),
                None => ok_check(category),
            },
        )
        .collect();
    DbHealthVerdict {
        checks,
        score: 0,
        score_version: "test",
    }
}

fn clean_signal(domain: HostDomain) -> DomainSignal {
    DomainSignal {
        domain,
        tier: analysis::thresholds::Tier::Normal,
        sample_count: 10,
        crossed_count: 0,
    }
}

fn crossed_signal(domain: HostDomain, sample_count: usize, crossed_count: usize) -> DomainSignal {
    DomainSignal {
        domain,
        tier: analysis::thresholds::Tier::High,
        sample_count,
        crossed_count,
    }
}

/// A host verdict with the given domain signals and no evidence — the
/// evidence-filtering tests below build their own verdicts with real
/// evidence entries instead.
fn host_verdict(signals: Vec<DomainSignal>) -> HostVerdict {
    HostVerdict {
        bottleneck: HostBottleneck::Unknown,
        domain_signals: signals,
        evidence: Vec::new(),
        score: 0,
        score_version: "test",
    }
}

fn clean_host() -> HostVerdict {
    host_verdict(vec![
        clean_signal(HostDomain::Cpu),
        clean_signal(HostDomain::Memory),
        clean_signal(HostDomain::StorageIo),
        clean_signal(HostDomain::Network),
    ])
}

fn has_cause(causes: &[RootCause], cause: RootCause) -> bool {
    causes.contains(&cause)
}

#[test]
fn db_locks_crossed_ranks_highest_and_all_host_causes_are_ruled_out() {
    let db = db_verdict(&[
        (DbHealthCategory::LockWaits, DbHealthStatus::Critical),
        (DbHealthCategory::Deadlocks, DbHealthStatus::Critical),
        (DbHealthCategory::LongTransactions, DbHealthStatus::Critical),
    ]);
    let host = clean_host();
    let now = Utc::now();

    let result = correlate(&host, &db, now, now);

    let ranked_causes: Vec<RootCause> = result.ranked.iter().map(|h| h.cause).collect();
    assert_eq!(ranked_causes, vec![RootCause::DbLocks]);
    assert_eq!(result.ranked[0].confidence, 1.0);
    assert!(!result.ranked[0].evidence.is_empty());

    let ruled_out_causes: Vec<RootCause> = result.ruled_out.iter().map(|r| r.cause).collect();
    for host_cause in [
        RootCause::HostCpu,
        RootCause::HostMemory,
        RootCause::StorageLatency,
        RootCause::Network,
    ] {
        assert!(
            has_cause(&ruled_out_causes, host_cause),
            "{host_cause:?} must be ruled out when the host is clean"
        );
    }
    assert!(has_cause(
        &ruled_out_causes,
        RootCause::ClientSideApplication
    ));
}

#[test]
fn a_crossed_host_domain_and_a_crossed_db_check_both_rank_the_real_cross_layer_case() {
    let db = db_verdict(&[
        (DbHealthCategory::Latency, DbHealthStatus::Critical),
        (DbHealthCategory::SlowQueries, DbHealthStatus::Critical),
    ]);
    let host = host_verdict(vec![
        crossed_signal(HostDomain::StorageIo, 10, 8),
        clean_signal(HostDomain::Cpu),
        clean_signal(HostDomain::Memory),
        clean_signal(HostDomain::Network),
    ]);
    let now = Utc::now();

    let result = correlate(&host, &db, now, now);
    let ranked_causes: Vec<RootCause> = result.ranked.iter().map(|h| h.cause).collect();

    assert!(
        has_cause(&ranked_causes, RootCause::SlowSql),
        "the DB's own slow-query signal must rank"
    );
    assert!(
        has_cause(&ranked_causes, RootCause::StorageLatency),
        "the host's own storage-latency signal must rank"
    );
    let storage = result
        .ranked
        .iter()
        .find(|h| h.cause == RootCause::StorageLatency)
        .expect("StorageLatency present");
    assert_eq!(storage.confidence, 0.8);
}

#[test]
fn an_unmonitored_db_rules_out_every_db_cause_with_no_db_evidence_available() {
    let db = analysis::unavailable_verdict("no connection configured", Utc::now());
    let host = host_verdict(vec![
        crossed_signal(HostDomain::Cpu, 5, 5),
        clean_signal(HostDomain::Memory),
        clean_signal(HostDomain::StorageIo),
        clean_signal(HostDomain::Network),
    ]);
    let now = Utc::now();

    let result = correlate(&host, &db, now, now);

    for db_cause in [
        RootCause::DbLocks,
        RootCause::DbConfiguration,
        RootCause::ConnectionExhaustion,
        RootCause::SlowSql,
    ] {
        let ruled_out = result
            .ruled_out
            .iter()
            .find(|r| r.cause == db_cause)
            .unwrap_or_else(|| panic!("{db_cause:?} must be ruled out"));
        assert_eq!(ruled_out.reason, "no DB evidence available");
    }

    let ranked_causes: Vec<RootCause> = result.ranked.iter().map(|h| h.cause).collect();
    assert_eq!(ranked_causes, vec![RootCause::HostCpu]);
    assert_eq!(result.ranked[0].confidence, 1.0);
}

#[test]
fn nothing_crossed_anywhere_leaves_ranked_empty() {
    let db = db_verdict(&[]);
    let host = clean_host();
    let now = Utc::now();

    let result = correlate(&host, &db, now, now);

    assert!(
        result.ranked.is_empty(),
        "no crossed signal anywhere must leave nothing ranked, got {:?}",
        result.ranked.iter().map(|h| h.cause).collect::<Vec<_>>()
    );
    assert_eq!(result.ruled_out.len(), RootCause::EVIDENCED.len() + 1);
}

#[test]
fn a_moderate_unresolved_db_signal_makes_client_side_application_a_live_hypothesis_too() {
    // A single Warning-level check (severity 0.5), `SlowQueries` marked
    // Unavailable (excluded from the average, not counted as Ok) so
    // SlowSql's confidence is exactly 0.5, not diluted by an assumed-Ok
    // sibling check: client-side = 0.5 * (1.0 - 0.5) = 0.25 — real,
    // uncertain-enough-to-consider-both territory.
    let db = db_verdict(&[
        (DbHealthCategory::Latency, DbHealthStatus::Warning),
        (DbHealthCategory::SlowQueries, DbHealthStatus::Unavailable),
    ]);
    let host = clean_host();
    let now = Utc::now();

    let result = correlate(&host, &db, now, now);
    let ranked_causes: Vec<RootCause> = result.ranked.iter().map(|h| h.cause).collect();

    assert!(has_cause(&ranked_causes, RootCause::SlowSql));
    assert!(has_cause(&ranked_causes, RootCause::ClientSideApplication));

    let slow_sql_confidence = result
        .ranked
        .iter()
        .find(|h| h.cause == RootCause::SlowSql)
        .expect("SlowSql present")
        .confidence;
    let client_side_confidence = result
        .ranked
        .iter()
        .find(|h| h.cause == RootCause::ClientSideApplication)
        .expect("ClientSideApplication present")
        .confidence;
    assert_eq!(slow_sql_confidence, 0.5);
    assert_eq!(client_side_confidence, 0.25);
    assert!(
        slow_sql_confidence > client_side_confidence,
        "ranked must sort descending by confidence"
    );
    assert_eq!(result.ranked[0].cause, RootCause::SlowSql);
}
