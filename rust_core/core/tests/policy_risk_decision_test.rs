//! Exhaustive coverage of `policy::decide` (SRS FR-POL-001/002/004).
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::policy::{decide, Environment, PolicyDecision, RiskLevel};

const ALL_RISK: [RiskLevel; 5] = [
    RiskLevel::Informational,
    RiskLevel::Low,
    RiskLevel::Medium,
    RiskLevel::High,
    RiskLevel::Prohibited,
];
const ALL_ENV: [Environment; 3] = [
    Environment::Development,
    Environment::Staging,
    Environment::Production,
];

#[test]
fn prohibited_is_always_denied() {
    for env in ALL_ENV {
        assert_eq!(decide(RiskLevel::Prohibited, env), PolicyDecision::Deny);
    }
}

#[test]
fn informational_is_always_auto_allowed() {
    for env in ALL_ENV {
        assert_eq!(
            decide(RiskLevel::Informational, env),
            PolicyDecision::AutoAllow
        );
    }
}

#[test]
fn medium_and_high_always_require_approval() {
    for risk in [RiskLevel::Medium, RiskLevel::High] {
        for env in ALL_ENV {
            assert_eq!(decide(risk, env), PolicyDecision::RequireApproval);
        }
    }
}

/// SRS FR-POL-002: risk level plus environment determines the decision.
#[test]
fn low_risk_requires_approval_only_in_production() {
    assert_eq!(
        decide(RiskLevel::Low, Environment::Development),
        PolicyDecision::AutoAllow
    );
    assert_eq!(
        decide(RiskLevel::Low, Environment::Staging),
        PolicyDecision::AutoAllow
    );
    assert_eq!(
        decide(RiskLevel::Low, Environment::Production),
        PolicyDecision::RequireApproval
    );
}

/// SRS FR-POL-004: production is never more permissive than development.
#[test]
fn production_is_never_more_permissive_than_development_fr_pol_004() {
    for risk in ALL_RISK {
        let dev = decide(risk, Environment::Development);
        let prod = decide(risk, Environment::Production);
        assert!(
            prod <= dev,
            "risk {risk:?}: production ({prod:?}) must never be more permissive than development ({dev:?})"
        );
    }
}

#[test]
fn staging_is_never_more_permissive_than_development() {
    for risk in ALL_RISK {
        let dev = decide(risk, Environment::Development);
        let staging = decide(risk, Environment::Staging);
        assert!(staging <= dev);
    }
}

#[test]
fn every_risk_by_environment_combination_produces_a_decision() {
    let mut count = 0;
    for risk in ALL_RISK {
        for env in ALL_ENV {
            let _ = decide(risk, env);
            count += 1;
        }
    }
    assert_eq!(count, 15);
}
