//! `core` — telemetry parsing, persistence, performance analysis,
//! correlation, policy, actions, AI integration, and security detection
//! (docs/TRS.md §3).
//!
//! **Naming note:** this package is named `core` per TRS §3, which collides
//! with Rust's own sysroot `core` crate. Any future dependent must import it
//! under an alias to avoid an ambiguous bare `core::…` path:
//!
//! ```toml
//! # in a dependent crate's Cargo.toml
//! ai_ops_core = { package = "core", path = "../core" }
//! ```
//!
//! ```rust,ignore
//! use ai_ops_core::...; // not `use core::...;`
//! ```
//!
//! See docs/adr/0003-workspace-layout-four-crates.md.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

// `/proc`/`/sys` parsing has no meaning off Linux; gated so the CI
// cross-target `cargo check` for Windows/macOS stays trivial (unit U1).
#[cfg(target_os = "linux")]
pub mod telemetry;
