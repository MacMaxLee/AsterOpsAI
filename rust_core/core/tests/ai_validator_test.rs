//! Fixture-based coverage of `ai::validator::validate` (SRS FR-AI-004/005):
//! every citation-failure mode, and proof that one bad citation among many
//! good ones discards the *whole* response, not just the bad part.
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::ai::{
    validator::validate, Candidate, EvidenceBundle, EvidenceItem, MetricClaim, Observation,
    RawAiExplanation, Recommendation,
};

fn bundle() -> EvidenceBundle {
    EvidenceBundle {
        subject: "HOST".to_string(),
        verdict_label: "CPU".to_string(),
        evidence: vec![
            EvidenceItem {
                id: 0,
                metric: "cpu_pressure_sustained_fraction".to_string(),
                observed: 0.8,
                threshold: 0.6,
                unit: None,
            },
            EvidenceItem {
                id: 1,
                metric: "background_service_share".to_string(),
                observed: 0.7,
                threshold: 0.5,
                unit: None,
            },
        ],
        candidates: vec![Candidate {
            id: 0,
            kind: "process".to_string(),
            label: "pid 1234 (chromium)".to_string(),
        }],
    }
}

fn well_formed_response() -> RawAiExplanation {
    RawAiExplanation {
        summary: "CPU is under sustained pressure.".to_string(),
        observations: vec![Observation {
            text: "CPU pressure crossed the sustained threshold.".to_string(),
            metrics: vec![MetricClaim {
                value: 0.8,
                evidence_ref: 0,
            }],
        }],
        recommendations: vec![Recommendation {
            text: "Investigate the background process.".to_string(),
            metrics: vec![MetricClaim {
                value: 0.7,
                evidence_ref: 1,
            }],
            candidate_ref: Some(0),
        }],
        risk: "MEDIUM".to_string(),
        confidence: 0.75,
    }
}

#[test]
fn well_formed_response_validates() {
    let result = validate(well_formed_response(), &bundle());
    assert!(result.is_ok(), "{:?}", result.err());
    let explanation = result.unwrap();
    assert_eq!(explanation.confidence, 0.75);
}

/// SRS FR-AI-003: any field outside the closed schema is discarded, not
/// an error — a real `serde_json::from_str` round-trip, not just a struct
/// literal, since that's the only way to prove deserialization actually
/// tolerates the extra field rather than merely never being given one.
#[test]
fn a_field_outside_the_closed_schema_is_discarded_not_an_error() {
    let json = r#"{
        "summary": "CPU is under sustained pressure.",
        "observations": [],
        "recommendations": [],
        "risk": "MEDIUM",
        "confidence": 0.75,
        "root_cause_pid": 1234
    }"#;
    let parsed: RawAiExplanation =
        serde_json::from_str(json).expect("unknown field must not fail parsing");
    assert_eq!(parsed.confidence, 0.75);
}

/// SRS FR-AI-005: an unresolved numeric evidence_ref discards the whole response.
#[test]
fn unresolvable_evidence_ref_in_observation_rejects_whole_response() {
    let mut raw = well_formed_response();
    raw.observations[0].metrics[0].evidence_ref = 99;
    let result = validate(raw, &bundle());
    assert!(result.is_err());
}

#[test]
fn one_bad_citation_among_many_good_ones_rejects_whole_response() {
    let mut raw = well_formed_response();
    // Add a second, valid observation alongside the one that will be bad,
    // and a second valid recommendation — proving the failure isn't scoped
    // to just the offending item.
    raw.observations.push(Observation {
        text: "A second, entirely valid observation.".to_string(),
        metrics: vec![MetricClaim {
            value: 0.8,
            evidence_ref: 0,
        }],
    });
    raw.recommendations[0].metrics.push(MetricClaim {
        value: 123.0,
        evidence_ref: 42, // the one bad citation
    });
    let result = validate(raw, &bundle());
    assert!(
        result.is_err(),
        "one bad citation must discard the whole response, not just the bad claim"
    );
}

#[test]
fn unresolvable_candidate_ref_rejects_whole_response() {
    let mut raw = well_formed_response();
    raw.recommendations[0].candidate_ref = Some(99);
    let result = validate(raw, &bundle());
    assert!(result.is_err());
}

#[test]
fn null_candidate_ref_is_fine() {
    let mut raw = well_formed_response();
    raw.recommendations[0].candidate_ref = None;
    let result = validate(raw, &bundle());
    assert!(result.is_ok());
}

#[test]
fn confidence_out_of_range_is_rejected() {
    for bad in [-0.1, 1.1, f64::NAN] {
        let mut raw = well_formed_response();
        raw.confidence = bad;
        assert!(
            validate(raw, &bundle()).is_err(),
            "confidence {bad} should be rejected"
        );
    }
}

#[test]
fn confidence_at_exact_boundaries_is_accepted() {
    for boundary in [0.0, 1.0] {
        let mut raw = well_formed_response();
        raw.confidence = boundary;
        assert!(validate(raw, &bundle()).is_ok());
    }
}

#[test]
fn unrecognized_risk_level_is_rejected() {
    let mut raw = well_formed_response();
    raw.risk = "SUPER_DUPER_BAD".to_string();
    assert!(validate(raw, &bundle()).is_err());
}

#[test]
fn every_allowed_risk_level_is_accepted() {
    for risk in ["LOW", "MEDIUM", "HIGH", "CRITICAL"] {
        let mut raw = well_formed_response();
        raw.risk = risk.to_string();
        assert!(
            validate(raw, &bundle()).is_ok(),
            "{risk} should be accepted"
        );
    }
}

#[test]
fn empty_observations_and_recommendations_is_fine() {
    let mut raw = well_formed_response();
    raw.observations.clear();
    raw.recommendations.clear();
    assert!(validate(raw, &bundle()).is_ok());
}
