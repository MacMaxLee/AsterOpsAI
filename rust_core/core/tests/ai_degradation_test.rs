//! Proves SRS FR-AI-001 / TRS §22's resilience requirement for
//! `ai::try_explain`: absent, timed-out, and garbage-output providers all
//! degrade to `None` — never a panic, never an unbounded block. Every
//! other subsystem calling through this entry point keeps working
//! regardless of what the AI provider does.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "ai/common/mod.rs"]
mod common;

use std::time::Duration;

use ai_ops_core::ai::{try_explain, AiProviderConfig, EvidenceBundle, OllamaProvider};
use common::{closed_port, http_ok_json_body, http_status_no_body, slow_one_shot_server};

fn bundle() -> EvidenceBundle {
    EvidenceBundle {
        subject: "HOST".to_string(),
        verdict_label: "NONE".to_string(),
        evidence: Vec::new(),
        candidates: Vec::new(),
    }
}

fn config_for_port(port: u16, timeout: Duration) -> AiProviderConfig {
    let mut config = AiProviderConfig::new("test-model");
    config.base_url = format!("http://127.0.0.1:{port}");
    config.timeout = timeout;
    config
}

#[tokio::test]
async fn absent_provider_degrades_to_none() {
    let port = closed_port().await;
    let provider = OllamaProvider::new(config_for_port(port, Duration::from_secs(3)));
    let result = try_explain(&provider, &bundle()).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn slow_provider_degrades_to_none_within_the_configured_timeout() {
    let port = slow_one_shot_server(Duration::from_secs(5), http_ok_json_body("{}")).await;
    let provider = OllamaProvider::new(config_for_port(port, Duration::from_millis(150)));

    let started = tokio::time::Instant::now();
    let result = try_explain(&provider, &bundle()).await;
    let elapsed = started.elapsed();

    assert!(result.is_none());
    assert!(
        elapsed < Duration::from_secs(1),
        "try_explain must not block past the configured timeout, took {elapsed:?}"
    );
}

#[tokio::test]
async fn garbage_http_status_degrades_to_none() {
    let port = common::one_shot_server(http_status_no_body(503, "Service Unavailable")).await;
    let provider = OllamaProvider::new(config_for_port(port, Duration::from_secs(3)));
    let result = try_explain(&provider, &bundle()).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn malformed_json_degrades_to_none() {
    let port = common::one_shot_server(http_ok_json_body("not json")).await;
    let provider = OllamaProvider::new(config_for_port(port, Duration::from_secs(3)));
    let result = try_explain(&provider, &bundle()).await;
    assert!(result.is_none());
}
