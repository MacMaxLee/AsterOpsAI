//! `core` — telemetry parsing, persistence, performance analysis,
//! correlation, policy, actions, AI integration, and security detection
//! (docs/TRS.md §3).
//!
//! **Naming note:** this package is named `core` per TRS §3, which collides
//! with Rust's own sysroot `core` crate. The fix lives in `Cargo.toml`'s
//! `[lib] name = "ai_ops_core"` — the on-disk package/directory stays
//! `core`, but the compiled library's own crate name is `ai_ops_core`
//! everywhere: dependents import it as such, and (just as importantly) this
//! crate's own `tests/*.rs` integration tests and doctests — which Cargo
//! compiles as separate binaries that self-`--extern` the crate under its
//! own name — no longer shadow the sysroot `core` inside those binaries
//! either. Any future dependent imports it explicitly:
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

// SQLite persistence works identically on every platform — NOT gated,
// unlike `telemetry` above (unit U2).
pub mod repository;

// Connects to a remote PostgreSQL server over the network; nothing here
// reads local OS files, so — like `repository` above — this isn't gated to
// Linux the way `telemetry` is (unit U4).
pub mod dbms;

// Pure classification/scoring over caller-supplied evidence (`contracts`
// types and `repository` row types, not raw `/proc`/`/sys` reads) — not
// gated to Linux, same reasoning as `dbms` above (unit U5).
pub mod analysis;

// Talks to a local/LAN Ollama endpoint over plain HTTP; reads no local OS
// files — not gated to Linux, same reasoning as `dbms`/`analysis` above
// (unit U6).
pub mod ai;

// Pure decision/typestate logic, no OS interaction of its own — not gated
// to Linux, same reasoning as `dbms`/`analysis`/`ai` above (unit U7).
pub mod policy;

// The `ActionKind`/`TargetVerifier` traits have no OS-specific code in
// `core/src` (zero production implementations ship this unit) — not gated
// to Linux for the same reason `policy` above isn't (unit U7).
pub mod actions;

// Statistics/orchestration are pure logic; only its one real
// `MetricSampler` implementation (`sampler::HostCpuUtilizationSampler`) is
// internally Linux-gated, same pattern as `actions::host` (unit U9).
pub mod benchmark;
