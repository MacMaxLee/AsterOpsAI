//! OS-specific logic behind one trait, [`PlatformAdapter`]. This is the only
//! crate in the workspace permitted `unsafe` code, and `platform/*/exec.rs`
//! are the only modules permitted to call `std::process::Command` — both
//! enforced by CI grep gates (`scripts/check-no-command-outside-exec.sh`),
//! not just convention. See docs/adr/0003-workspace-layout-four-crates.md.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod adapter;
pub mod error;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub use adapter::{CpuAffinityMask, PlatformAdapter, ProcessPriority, ProcessSelfMetrics};
pub use error::CapabilityError;

/// Selects the `PlatformAdapter` implementation for the OS this binary was
/// compiled for.
pub fn current_platform_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxPlatformAdapter)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatformAdapter)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosPlatformAdapter)
    }
}
