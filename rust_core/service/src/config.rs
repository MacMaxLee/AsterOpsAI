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
///
/// `std::env::var` returns `Ok("")` for a variable that's exported but
/// empty, not an error — an empty value is treated the same as unset here
/// (`.filter(|v| !v.is_empty())`), or it would resolve relative to the
/// process's CWD instead of failing fast the way this promises to.
///
/// Pure and independently testable from the real lookup: `resolve_
/// default_db_path` (the `#[cfg(unix)]` public entry point below) just
/// wires the raw env values into this. `service`'s `unsafe_code = "forbid"`
/// lint means tests can't mutate real process env vars via `std::env::
/// set_var` (unconditionally `unsafe`) to exercise the empty-vs-unset
/// distinction — this shape sidesteps that rather than needing an
/// exception to a `forbid` lint, which can't be granted locally anyway.
fn resolve_default_db_path_from(
    xdg_data_home: Option<String>,
    home: Option<String>,
) -> anyhow::Result<PathBuf> {
    let non_empty = |v: Option<String>| v.filter(|s| !s.is_empty());

    let data_home = if let Some(dir) = non_empty(xdg_data_home) {
        PathBuf::from(dir)
    } else {
        let home = non_empty(home).ok_or_else(|| {
            anyhow::anyhow!(
                "neither XDG_DATA_HOME nor HOME is set; refusing to guess a database location"
            )
        })?;
        PathBuf::from(home).join(".local").join("share")
    };
    Ok(data_home.join("ai-ops-coordinator").join("core.sqlite3"))
}

#[cfg(unix)]
pub fn resolve_default_db_path() -> anyhow::Result<PathBuf> {
    resolve_default_db_path_from(
        std::env::var("XDG_DATA_HOME").ok(),
        std::env::var("HOME").ok(),
    )
}

/// `%APPDATA%\ai-ops-coordinator\core.sqlite3`. Only needs to type-check
/// until unit U12's real Windows support lands (DoD only runs `cargo check`
/// for this target).
#[cfg(windows)]
pub fn resolve_default_db_path() -> anyhow::Result<PathBuf> {
    let app_data = std::env::var("APPDATA")
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("APPDATA is not set; refusing to guess a database location")
        })?;
    Ok(PathBuf::from(app_data)
        .join("ai-ops-coordinator")
        .join("core.sqlite3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for a real bug found by a full-codebase scan: the
    /// original code used `if let Ok(dir) = std::env::var(...)`, which
    /// treats an exported-but-empty variable as "set" — an empty
    /// `XDG_DATA_HOME` (with no `HOME` fallback) should fail fast, not
    /// resolve to a path relative to the process's CWD.
    #[test]
    fn an_empty_xdg_data_home_with_no_home_fallback_is_an_error() {
        let result = resolve_default_db_path_from(Some(String::new()), None);
        assert!(result.is_err());
    }

    #[test]
    fn an_empty_xdg_data_home_falls_back_to_a_non_empty_home() {
        let result = resolve_default_db_path_from(Some(String::new()), Some("/home/x".into()));
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/home/x/.local/share/ai-ops-coordinator/core.sqlite3")
        );
    }

    #[test]
    fn a_real_xdg_data_home_is_used_directly() {
        let result = resolve_default_db_path_from(Some("/data".into()), None);
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/data/ai-ops-coordinator/core.sqlite3")
        );
    }

    #[test]
    fn neither_set_is_an_error() {
        assert!(resolve_default_db_path_from(None, None).is_err());
    }

    #[test]
    fn an_empty_home_with_no_xdg_data_home_is_an_error() {
        let result = resolve_default_db_path_from(None, Some(String::new()));
        assert!(result.is_err());
    }
}
