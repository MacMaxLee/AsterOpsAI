//! `core` — telemetry parsing, persistence, performance analysis,
//! correlation, policy, actions, AI integration, and security detection
//! (docs/TRS.md §3). Intentionally empty in unit U0; the first real modules
//! land in U1.
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
