//! SRS FR-SYS-001 (unit U64): `transport::unix::serve` exercised for
//! real — every other service integration test hits the router
//! in-process (`tower::ServiceExt::oneshot`), never through the actual
//! Unix Domain Socket transport this binary uses in production. Confirms
//! the real bind, the real `0600` permission bits (docs/adr/0001), and a
//! real client genuinely connecting and getting a real HTTP response
//! back over that socket.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use axum::routing::get;
use axum::Router;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn serve_binds_mode_0600_and_a_real_client_connects_over_the_real_socket() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket_path = dir.path().join("core.sock");

    let app = Router::new().route("/ping", get(|| async { "pong" }));
    let serve_path = socket_path.clone();
    tokio::spawn(async move {
        service::transport::unix::serve(app, &serve_path)
            .await
            .expect("serve");
    });

    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(socket_path.exists(), "socket file was never created");

    let mode = tokio::fs::metadata(&socket_path)
        .await
        .expect("stat socket")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    let mut stream = UnixStream::connect(&socket_path)
        .await
        .expect("connect over the real unix domain socket");
    stream
        .write_all(b"GET /ping HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("write request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .expect("read response");
    let response = String::from_utf8_lossy(&response);
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected status line: {response}"
    );
    assert!(
        response.contains("pong"),
        "expected the real router's response body, got: {response}"
    );
}
