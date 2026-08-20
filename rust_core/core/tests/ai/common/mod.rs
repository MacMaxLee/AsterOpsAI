//! A real local TCP server this test suite controls, standing in for a
//! real Ollama daemon (not installed in this sandbox — see docs/adr/0011).
//! `core::ai::OllamaProvider` is exercised against real sockets and real
//! bytes here, never a mocked `AiProvider` trait impl.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Binds an ephemeral localhost port, accepts exactly one connection,
/// reads (and discards) whatever the client sends, writes
/// `response_bytes` verbatim, then closes. Returns the port to point the
/// real client at.
pub async fn one_shot_server(response_bytes: Vec<u8>) -> u16 {
    slow_one_shot_server(Duration::ZERO, response_bytes).await
}

/// Like [`one_shot_server`] but waits `delay` after reading the request
/// before writing the response — for timeout tests.
pub async fn slow_one_shot_server(delay: Duration, response_bytes: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await;
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            let _ = socket.write_all(&response_bytes).await;
            let _ = socket.shutdown().await;
        }
    });
    port
}

/// Binds an ephemeral port and immediately drops the listener — connecting
/// to the returned port afterward reliably yields a real
/// connection-refused error, no server double needed.
pub async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

pub fn http_ok_json_body(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

pub fn http_status_no_body(status: u16, reason: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .into_bytes()
}

/// Wraps a `RawAiExplanation`-shaped JSON string the same way Ollama's real
/// `/api/generate` response does: the model's completion is itself a JSON
/// *string* nested inside the `response` field, not raw nested JSON.
pub fn ollama_envelope(inner_json: &str) -> String {
    serde_json::json!({ "response": inner_json, "done": true }).to_string()
}
