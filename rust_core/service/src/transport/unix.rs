//! Unix Domain Socket transport (Linux/macOS). Deliberately not loopback
//! TCP, so the local API is not reachable from the network or other local
//! users by default — see docs/adr/0001-transport-uds-named-pipe-not-tcp.md.
//!
//! axum 0.7's `axum::serve` only accepts a `TcpListener`, so connections are
//! accepted manually and handed to hyper directly.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use axum::Router;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;

/// Resolves the socket path under `$XDG_RUNTIME_DIR`. Fails fast rather than
/// silently falling back to `/tmp` when the variable is unset — a
/// predictable, permission-correct location matters more than best-effort
/// availability.
pub fn resolve_socket_path() -> anyhow::Result<PathBuf> {
    let dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| {
        anyhow::anyhow!(
            "XDG_RUNTIME_DIR is not set; refusing to guess a socket location. \
             Set it (e.g. to /run/user/<uid>) before starting the service."
        )
    })?;
    Ok(PathBuf::from(dir)
        .join("ai-ops-coordinator")
        .join("core.sock"))
}

/// Binds `socket_path` mode 0600, removing a stale socket from a prior run
/// first, and serves `app` over it.
pub async fn serve(app: Router, socket_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if socket_path.exists() {
        tokio::fs::remove_file(socket_path).await?;
    }

    let listener = tokio::net::UnixListener::bind(socket_path)?;
    tokio::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600)).await?;
    tracing::info!(path = %socket_path.display(), "listening on unix domain socket");

    let service = TowerToHyperService::new(app);
    loop {
        let (stream, _addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                tracing::warn!(error = %err, "connection error");
            }
        });
    }
}
