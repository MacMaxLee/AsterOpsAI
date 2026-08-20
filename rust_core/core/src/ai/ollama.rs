//! The one real `AiProvider` implementation: a plain HTTP/1.1 client built
//! directly on `hyper`'s client feature (see docs/adr/0011 for why not
//! `reqwest`/TLS), talking to Ollama's `/api/generate` endpoint. A fresh
//! `TcpStream` + handshake per call — no connection pool — since these
//! calls are infrequent and a one-shot connection keeps the client trivial.

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use super::bundle::EvidenceBundle;
use super::prompt::{build_system_prompt, build_user_prompt};
use super::provider::{AiError, AiProvider, AiProviderConfig};
use super::schema::{AiExplanation, RawAiExplanation};
use super::validator::validate;

pub struct OllamaProvider {
    config: AiProviderConfig,
}

impl OllamaProvider {
    pub fn new(config: AiProviderConfig) -> Self {
        Self { config }
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    system: &'a str,
    prompt: String,
    stream: bool,
    format: &'a str,
}

/// Ollama's real, documented `/api/generate` (non-streaming) envelope —
/// `response` is the model's completion as a JSON *string*, not nested
/// JSON, hence the second `serde_json::from_str` below. Every other field
/// Ollama sends is ignored by plain `Deserialize`, matching the same
/// "discard, don't reject" behavior `schema::RawAiExplanation` relies on.
#[derive(Deserialize)]
struct GenerateResponseEnvelope {
    response: String,
}

/// `base_url` is deliberately restricted to `http://host:port` — no path,
/// no TLS scheme (see docs/adr/0011's no-TLS scope-down).
fn parse_base_url(base_url: &str) -> Result<(String, u16), AiError> {
    let without_scheme = base_url.strip_prefix("http://").ok_or_else(|| {
        AiError::Connect(format!(
            "only http:// base URLs are supported, got {base_url:?}"
        ))
    })?;
    let host_port = without_scheme
        .split_once('/')
        .map_or(without_scheme, |(host_port, _rest)| host_port);
    let (host, port) = host_port.split_once(':').ok_or_else(|| {
        AiError::Connect(format!("base_url must include a port, got {base_url:?}"))
    })?;
    let port: u16 = port
        .parse()
        .map_err(|_| AiError::Connect(format!("invalid port in base_url {base_url:?}")))?;
    Ok((host.to_string(), port))
}

async fn call_ollama(
    host: &str,
    port: u16,
    config: &AiProviderConfig,
    bundle: &EvidenceBundle,
) -> Result<AiExplanation, AiError> {
    let stream = TcpStream::connect((host, port))
        .await
        .map_err(|e| AiError::Connect(e.to_string()))?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|e| AiError::Connect(e.to_string()))?;
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let request_body = GenerateRequest {
        model: &config.model,
        system: build_system_prompt(),
        prompt: build_user_prompt(bundle),
        stream: false,
        format: "json",
    };
    let body_bytes =
        serde_json::to_vec(&request_body).map_err(|e| AiError::InvalidJson(e.to_string()))?;

    let request = Request::builder()
        .method("POST")
        .uri("/api/generate")
        .header("Host", format!("{host}:{port}"))
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body_bytes)))
        .map_err(|e| AiError::Connect(e.to_string()))?;

    let response = sender
        .send_request(request)
        .await
        .map_err(|e| AiError::Connect(e.to_string()))?;
    let status = response.status();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| AiError::Connect(e.to_string()))?
        .to_bytes();

    if !status.is_success() {
        return Err(AiError::HttpStatus(status.as_u16()));
    }

    let envelope: GenerateResponseEnvelope =
        serde_json::from_slice(&body_bytes).map_err(|e| AiError::InvalidJson(e.to_string()))?;
    let raw: RawAiExplanation = serde_json::from_str(&envelope.response)
        .map_err(|e| AiError::InvalidJson(e.to_string()))?;
    validate(raw, bundle)
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn explain(&self, bundle: &EvidenceBundle) -> Result<AiExplanation, AiError> {
        let (host, port) = parse_base_url(&self.config.base_url)?;
        match tokio::time::timeout(
            self.config.timeout,
            call_ollama(&host, port, &self.config, bundle),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Err(AiError::Timeout),
        }
    }
}
