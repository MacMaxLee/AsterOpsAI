//! Real `hyper`-based `OllamaProvider` HTTP client, exercised against a
//! real local `TcpListener` test double (no Ollama installed in this
//! sandbox — see docs/adr/0011). Every case here is a real socket, real
//! bytes, real timeout — never a mocked `AiProvider` trait impl.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "ai/common/mod.rs"]
mod common;

use std::time::Duration;

use ai_ops_core::ai::{
    AiError, AiProvider, AiProviderConfig, EvidenceBundle, EvidenceItem, OllamaProvider,
};
use common::{
    closed_port, http_ok_json_body, http_status_no_body, ollama_envelope, slow_one_shot_server,
};

fn bundle() -> EvidenceBundle {
    EvidenceBundle {
        subject: "HOST".to_string(),
        verdict_label: "CPU".to_string(),
        evidence: vec![EvidenceItem {
            id: 0,
            metric: "cpu_pressure_sustained_fraction".to_string(),
            observed: 0.8,
            threshold: 0.6,
            unit: None,
        }],
        candidates: Vec::new(),
    }
}

fn config_for_port(port: u16) -> AiProviderConfig {
    let mut config = AiProviderConfig::new("test-model");
    config.base_url = format!("http://127.0.0.1:{port}");
    config.timeout = Duration::from_secs(3);
    config
}

fn well_formed_inner_json() -> String {
    serde_json::json!({
        "summary": "CPU is under sustained pressure.",
        "observations": [{"text": "sustained CPU pressure", "metrics": [{"value": 0.8, "evidence_ref": 0}]}],
        "recommendations": [],
        "risk": "MEDIUM",
        "confidence": 0.7
    })
    .to_string()
}

#[tokio::test]
async fn well_formed_response_is_accepted() {
    let body = ollama_envelope(&well_formed_inner_json());
    let port = common::one_shot_server(http_ok_json_body(&body)).await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let explanation = result.unwrap();
    assert_eq!(explanation.summary, "CPU is under sustained pressure.");
}

#[tokio::test]
async fn connection_refused_is_a_connect_error() {
    let port = closed_port().await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(matches!(result, Err(AiError::Connect(_))), "{result:?}");
}

#[tokio::test]
async fn a_slow_response_times_out() {
    let body = ollama_envelope(&well_formed_inner_json());
    let port = slow_one_shot_server(Duration::from_secs(5), http_ok_json_body(&body)).await;
    let mut config = config_for_port(port);
    config.timeout = Duration::from_millis(200);
    let provider = OllamaProvider::new(config);

    let result = provider.explain(&bundle()).await;
    assert!(matches!(result, Err(AiError::Timeout)), "{result:?}");
}

#[tokio::test]
async fn a_non_success_http_status_is_reported() {
    let port = common::one_shot_server(http_status_no_body(500, "Internal Server Error")).await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(
        matches!(result, Err(AiError::HttpStatus(500))),
        "{result:?}"
    );
}

#[tokio::test]
async fn malformed_outer_envelope_is_an_invalid_json_error() {
    let port = common::one_shot_server(http_ok_json_body("this is not json at all")).await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(matches!(result, Err(AiError::InvalidJson(_))), "{result:?}");
}

#[tokio::test]
async fn well_formed_envelope_with_garbage_inner_json_is_an_invalid_json_error() {
    let body = ollama_envelope("not valid json either");
    let port = common::one_shot_server(http_ok_json_body(&body)).await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(matches!(result, Err(AiError::InvalidJson(_))), "{result:?}");
}

#[tokio::test]
async fn inner_json_missing_required_schema_fields_is_an_invalid_json_error() {
    let inner = serde_json::json!({"summary": "no risk or confidence field here"}).to_string();
    let body = ollama_envelope(&inner);
    let port = common::one_shot_server(http_ok_json_body(&body)).await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(matches!(result, Err(AiError::InvalidJson(_))), "{result:?}");
}

#[tokio::test]
async fn inner_json_with_an_unresolvable_evidence_ref_is_a_schema_validation_error() {
    let inner = serde_json::json!({
        "summary": "made-up claim",
        "observations": [{"text": "bogus", "metrics": [{"value": 1.0, "evidence_ref": 999}]}],
        "recommendations": [],
        "risk": "LOW",
        "confidence": 0.5
    })
    .to_string();
    let body = ollama_envelope(&inner);
    let port = common::one_shot_server(http_ok_json_body(&body)).await;
    let provider = OllamaProvider::new(config_for_port(port));

    let result = provider.explain(&bundle()).await;
    assert!(
        matches!(result, Err(AiError::SchemaValidation(_))),
        "{result:?}"
    );
}
