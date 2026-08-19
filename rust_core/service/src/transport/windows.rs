//! Named Pipe transport (Windows). Only required to type-check in unit U0
//! (`cargo check --target x86_64-pc-windows-msvc`) — a real, verified
//! implementation lands in unit U12. Not loopback TCP, matching the Unix
//! transport's isolation guarantee — see
//! docs/adr/0001-transport-uds-named-pipe-not-tcp.md.

use axum::Router;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use tokio::net::windows::named_pipe::ServerOptions;

pub fn pipe_name() -> String {
    r"\\.\pipe\ai-ops-coordinator\core".to_string()
}

/// Serves `app` over a named pipe, accepting one connection at a time and
/// spawning a fresh pipe instance for the next client before handling the
/// current one — the standard `NamedPipeServer` idiom.
pub async fn serve(app: Router) -> anyhow::Result<()> {
    let pipe_name = pipe_name();
    let service = TowerToHyperService::new(app);

    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)?;
    tracing::info!(pipe = %pipe_name, "listening on named pipe");

    loop {
        server.connect().await?;
        let connected = server;
        server = ServerOptions::new().create(&pipe_name)?;

        let io = TokioIo::new(connected);
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                tracing::warn!(error = %err, "named pipe connection error");
            }
        });
    }
}
