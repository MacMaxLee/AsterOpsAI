//! Default paths for the pieces of configuration that need one so far. The
//! socket/pipe path is still resolved directly in
//! `transport::unix`/`transport::windows`; this module covers the SQLite
//! database's default location (unit U2), which — unlike the socket —
//! must live somewhere that survives a reboot, so it deliberately does
//! *not* share `transport::unix`'s `$XDG_RUNTIME_DIR`.

use std::path::PathBuf;

/// `$XDG_DATA_HOME/ai-ops-coordinator/core.sqlite3`, falling back to
/// `$HOME/.local/share/...` per the XDG Base Directory spec when
/// `XDG_DATA_HOME` isn't set. Unlike the transport socket path, this is
/// never allowed to silently fall back to a temp directory — a database
/// that can vanish on reboot defeats the point of persisting it.
#[cfg(unix)]
pub fn resolve_default_db_path() -> anyhow::Result<PathBuf> {
    let data_home = if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME").map_err(|_| {
            anyhow::anyhow!(
                "neither XDG_DATA_HOME nor HOME is set; refusing to guess a database location"
            )
        })?;
        PathBuf::from(home).join(".local").join("share")
    };
    Ok(data_home.join("ai-ops-coordinator").join("core.sqlite3"))
}

/// `%APPDATA%\ai-ops-coordinator\core.sqlite3`. Only needs to type-check
/// until unit U12's real Windows support lands (DoD only runs `cargo check`
/// for this target).
#[cfg(windows)]
pub fn resolve_default_db_path() -> anyhow::Result<PathBuf> {
    let app_data = std::env::var("APPDATA").map_err(|_| {
        anyhow::anyhow!("APPDATA is not set; refusing to guess a database location")
    })?;
    Ok(PathBuf::from(app_data)
        .join("ai-ops-coordinator")
        .join("core.sqlite3"))
}
