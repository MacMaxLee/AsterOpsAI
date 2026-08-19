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

/// `std::env::var` returns `Ok("")` for a variable that's exported but
/// empty, not an error — `.filter(|d| !d.is_empty())` treats that the same
/// as unset, or an empty `XDG_RUNTIME_DIR` would resolve relative to the
/// process's CWD instead of failing fast the way this promises to.
///
/// Pure and independently testable from the real env lookup — see
/// `config::resolve_default_db_path_from`'s doc comment for why (this
/// crate's `unsafe_code = "forbid"` lint rules out testing via real
/// `std::env::set_var`, which is unconditionally `unsafe`).
fn resolve_socket_path_from(xdg_runtime_dir: Option<String>) -> anyhow::Result<PathBuf> {
    let dir = xdg_runtime_dir.filter(|d| !d.is_empty()).ok_or_else(|| {
        anyhow::anyhow!(
            "XDG_RUNTIME_DIR is not set; refusing to guess a socket location. \
             Set it (e.g. to /run/user/<uid>) before starting the service."
        )
    })?;
    Ok(PathBuf::from(dir)
        .join("ai-ops-coordinator")
        .join("core.sock"))
}

/// Resolves the socket path under `$XDG_RUNTIME_DIR`. Fails fast rather than
/// silently falling back to `/tmp` when the variable is unset — a
/// predictable, permission-correct location matters more than best-effort
/// availability.
pub fn resolve_socket_path() -> anyhow::Result<PathBuf> {
    resolve_socket_path_from(std::env::var("XDG_RUNTIME_DIR").ok())
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
        // A single failed accept() (e.g. transient EMFILE/ECONNABORTED) must
        // never take the whole listen loop down with it via `?` — that would
        // kill every other already-connected client's ability to be served
        // again, not just this one failed accept. Only a fatal *setup*
        // failure (the bind() above) should end `serve()`.
        let (stream, _addr) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(err) => {
                tracing::warn!(error = %err, "failed to accept a connection; continuing to listen");
                continue;
            }
        };
        let io = TokioIo::new(stream);
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                tracing::warn!(error = %err, "connection error");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for a real bug found by a full-codebase scan: `Ok(dir)
    /// = std::env::var(...)` treats an exported-but-empty variable as "set" —
    /// an empty `XDG_RUNTIME_DIR` should fail fast, not resolve to a path
    /// relative to the process's CWD.
    #[test]
    fn an_empty_xdg_runtime_dir_is_an_error() {
        assert!(resolve_socket_path_from(Some(String::new())).is_err());
    }

    #[test]
    fn unset_is_an_error() {
        assert!(resolve_socket_path_from(None).is_err());
    }

    #[test]
    fn a_real_value_resolves_under_it() {
        let result = resolve_socket_path_from(Some("/run/user/1000".into()));
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/run/user/1000/ai-ops-coordinator/core.sock")
        );
    }
}
