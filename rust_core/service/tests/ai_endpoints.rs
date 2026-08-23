//! Real end-to-end coverage of unit U44's first direct wire surface for
//! `core::ai` (fully built since unit U6, never previously reachable
//! over HTTP). A real axum router, a real SQLite-backed
//! `RepositoryHandle` (so `compute_host_verdict` genuinely runs), and a
//! real local TCP server standing in for a real Ollama daemon — the
//! exact same technique `core/tests/ai_ollama_provider_test.rs`'s own
//! doc comment establishes ("no Ollama installed in this sandbox," a
//! real socket/real bytes double instead) — not shareable across the
//! package boundary (ADR 0025's own constraint), so reimplemented here
//! directly rather than duplicated verbatim from a `core`-only helper.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use ai_ops_core::ai::{AiProvider, AiProviderConfig, OllamaProvider};
use ai_ops_core::dbms::adapters::postgresql::PostgresAdapter;
use ai_ops_core::dbms::{pool, DbmsAdapter};
use ai_ops_core::policy::{ActionTypeRegistry, Environment, ProtectedResourceRegistry};
use ai_ops_core::repository::{self, RepositoryConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use service::{api, self_metrics, state::AppState, telemetry};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::ServiceExt;

async fn build_app(
    repository: Option<repository::RepositoryHandle>,
    ai_provider: Option<Arc<dyn AiProvider>>,
    dbms_adapter: Option<Arc<dyn DbmsAdapter>>,
) -> axum::Router {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    // Decoupled from `repository` on purpose (same real discovery
    // `analysis_endpoints.rs`'s own `build_app_with_empty_repository`
    // already made): the sampler persists its first real snapshot
    // synchronously on its very first tick when handed `Some(repository)`,
    // so a genuinely-empty repository is only reachable by never handing
    // it to the sampler at all.
    let host_telemetry = telemetry::sampler::spawn(platform.clone(), None);
    let state = AppState::new(
        platform,
        self_metrics,
        host_telemetry,
        repository,
        dbms_adapter,
        Arc::new(ActionTypeRegistry::new()),
        Arc::new(ProtectedResourceRegistry::new()),
        Environment::Development,
        ai_provider,
    );
    api::router(state)
}

/// Mirrors `dbms_endpoints.rs`'s own `adapter_for` (test-only code,
/// not worth extracting into `support` for one caller).
fn adapter_for(pg: &support::TestPostgres) -> Arc<dyn DbmsAdapter> {
    let cfg = service::dbms_config::resolve_db_connection_from(
        Some(pg.socket_dir.display().to_string()),
        Some(pg.port.to_string()),
        Some("postgres".to_string()),
        Some("postgres".to_string()),
        Some("trust-auth-ignores-this".to_string()),
        Some("disable".to_string()),
        None,
        None,
    )
    .expect("host+password were both given");
    let pg_pool = pool::build_pool(&cfg.metadata, &cfg.password).expect("pool builds");
    Arc::new(PostgresAdapter::from_pool(pg_pool, false))
}

async fn open_repo() -> repository::RepositoryHandle {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.sqlite3");
    std::mem::forget(dir);
    repository::init(&RepositoryConfig { db_path }).expect("repository::init")
}

async fn get_json(app: axum::Router, path: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request builds");
    let response = app.oneshot(request).await.expect("router does not fail");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let json: Value = serde_json::from_slice(&body).expect("body is valid json");
    (status, json)
}

/// Binds an ephemeral localhost port, accepts exactly one connection,
/// reads (and discards) whatever the client sends, writes
/// `response_bytes` verbatim, then closes.
async fn one_shot_server(response_bytes: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await;
            let _ = socket.write_all(&response_bytes).await;
            let _ = socket.shutdown().await;
        }
    });
    port
}

/// Binds an ephemeral port and immediately drops the listener —
/// connecting afterward reliably yields a real connection-refused error.
async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

fn http_ok_json_body(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

/// Wraps a `RawAiExplanation`-shaped JSON string the same way Ollama's
/// real `/api/generate` response does: the model's completion is itself
/// a JSON *string* nested inside the `response` field.
fn ollama_envelope(inner_json: &str) -> String {
    serde_json::json!({ "response": inner_json, "done": true }).to_string()
}

fn provider_for_port(port: u16) -> Arc<dyn AiProvider> {
    let mut config = AiProviderConfig::new("test-model");
    config.base_url = format!("http://127.0.0.1:{port}");
    config.timeout = Duration::from_secs(3);
    Arc::new(OllamaProvider::new(config))
}

#[tokio::test]
async fn explain_host_is_unavailable_without_an_ai_provider() {
    let app = build_app(None, None, None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/host/explain").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

/// A provider *is* configured but genuinely unreachable (a closed port
/// — a real connection-refused error, not a guessed timeout) —
/// `try_explain`'s own degrade-to-`None` contract (SRS FR-AI-001) means
/// this is a real `200 OK` with a real `Unavailable` reason, not a
/// `503` — the feature itself is reachable, this one attempt just
/// didn't produce output, the same split `query_stats` (ADR 0038)
/// already established.
#[tokio::test]
async fn explain_host_reports_a_real_unavailable_state_as_200_ok_when_the_provider_is_unreachable()
{
    let repo = open_repo().await;
    let port = closed_port().await;
    let app = build_app(Some(repo), Some(provider_for_port(port)), None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/host/explain").await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "UNAVAILABLE");
    assert!(
        !body["data"]["reason"]
            .as_str()
            .expect("reason is a string")
            .is_empty(),
        "expected a real, non-empty reason, got: {body:?}"
    );
}

#[tokio::test]
async fn explain_host_returns_a_real_supported_explanation_from_a_real_provider_round_trip() {
    let repo = open_repo().await;
    let inner = serde_json::json!({
        "summary": "The host looks idle.",
        "observations": [],
        "recommendations": [],
        "risk": "LOW",
        "confidence": 0.9,
    })
    .to_string();
    let response_body = http_ok_json_body(&ollama_envelope(&inner));
    let port = one_shot_server(response_body).await;

    let app = build_app(Some(repo), Some(provider_for_port(port)), None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/host/explain").await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "SUPPORTED");
    assert_eq!(body["data"]["value"]["summary"], "The host looks idle.");
    assert_eq!(body["data"]["value"]["risk"], "LOW");
    assert_eq!(body["data"]["value"]["confidence"], 0.9);
}

#[tokio::test]
async fn explain_db_is_unavailable_without_an_ai_provider() {
    let app = build_app(None, None, None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/db/explain").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

/// The AI provider *is* configured (presence alone — `explain_db`
/// checks it first, before ever polling the database), but no
/// `dbms_adapter` is — a distinct real `503` reason from the
/// AI-provider case, matching `compute_db_verdict`'s own existing
/// "no database connection configured" wording exactly.
#[tokio::test]
async fn explain_db_is_unavailable_without_a_database() {
    let port = closed_port().await;
    let app = build_app(None, Some(provider_for_port(port)), None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/db/explain").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
    assert_eq!(
        body["error"]["message"], "no database connection configured",
        "expected the real, existing compute_db_verdict reason, got: {body:?}"
    );
}

#[tokio::test]
async fn explain_db_returns_a_real_supported_explanation_from_a_real_provider_and_database_round_trip(
) {
    let mut pg = support::TestPostgres::start(17).await;
    let dbms_adapter = adapter_for(&pg);
    let inner = serde_json::json!({
        "summary": "The database looks healthy.",
        "observations": [],
        "recommendations": [],
        "risk": "LOW",
        "confidence": 0.85,
    })
    .to_string();
    let response_body = http_ok_json_body(&ollama_envelope(&inner));
    let ai_port = one_shot_server(response_body).await;

    let app = build_app(None, Some(provider_for_port(ai_port)), Some(dbms_adapter)).await;
    let (status, body) = get_json(app, "/api/v1/analysis/db/explain").await;
    pg.stop().await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "SUPPORTED");
    assert_eq!(
        body["data"]["value"]["summary"],
        "The database looks healthy."
    );
    assert_eq!(body["data"]["value"]["risk"], "LOW");
    assert_eq!(body["data"]["value"]["confidence"], 0.85);
}

/// Unit U70: `explain_host`/`explain_db`'s third sibling. Mirrors
/// `explain_host_is_unavailable_without_an_ai_provider` exactly — the
/// provider check happens before `correlate()` is ever computed, so no
/// repository is needed here either.
#[tokio::test]
async fn explain_correlation_is_unavailable_without_an_ai_provider() {
    let app = build_app(None, None, None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation/explain").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
}

#[tokio::test]
async fn explain_correlation_reports_a_real_unavailable_state_as_200_ok_when_the_provider_is_unreachable(
) {
    let repo = open_repo().await;
    let port = closed_port().await;
    let app = build_app(Some(repo), Some(provider_for_port(port)), None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation/explain").await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "UNAVAILABLE");
    assert!(
        !body["data"]["reason"]
            .as_str()
            .expect("reason is a string")
            .is_empty(),
        "expected a real, non-empty reason, got: {body:?}"
    );
}

/// SRS FR-CORR-003: correlation output reaches the AI explanation layer
/// through the same `contracts` wire types the console renders — a real
/// end-to-end round trip through `/api/v1/analysis/correlation/explain`,
/// not just a unit test of `build_correlation_bundle` in isolation.
#[tokio::test]
async fn explain_correlation_returns_a_real_supported_explanation_from_a_real_provider_round_trip()
{
    let repo = open_repo().await;
    let inner = serde_json::json!({
        "summary": "No sustained root cause identified.",
        "observations": [],
        "recommendations": [],
        "risk": "LOW",
        "confidence": 0.7,
    })
    .to_string();
    let response_body = http_ok_json_body(&ollama_envelope(&inner));
    let port = one_shot_server(response_body).await;

    let app = build_app(Some(repo), Some(provider_for_port(port)), None).await;
    let (status, body) = get_json(app, "/api/v1/analysis/correlation/explain").await;

    assert_eq!(status, StatusCode::OK, "body: {body:?}");
    assert_eq!(body["data"]["state"], "SUPPORTED");
    assert_eq!(
        body["data"]["value"]["summary"],
        "No sustained root cause identified."
    );
    assert_eq!(body["data"]["value"]["risk"], "LOW");
    assert_eq!(body["data"]["value"]["confidence"], 0.7);
}
