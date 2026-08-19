# 0003 — Workspace Layout: `contracts` / `platform` / `core` / `service`

## Status

Accepted (unit U0).

## Context

The project needs OS-portable business logic cleanly separated from the wire
format and from the binary entry point, while anticipating v2's headless
`agent` and `coordinator` crates (TRS §28–29) without over-building for them
now. TRS §3 fixes the four crate names for v1.x and states this layout is not
restructured without an ADR.

## Decision

Four crates, each with one job:

- `contracts` — every wire type; zero workspace-internal dependencies.
- `platform` — the `PlatformAdapter` trait plus `linux/windows/macos`
  modules; the *only* crate permitted `unsafe` code, and `platform/*/exec.rs`
  the *only* modules permitted `std::process::Command` (both CI grep-gated).
- `core` — telemetry parsing, persistence, performance analysis,
  correlation, policy, actions, AI integration, security detection. Depends
  on `platform` + `contracts`; makes no direct OS calls of its own.
- `service` — the binary: HTTP-over-UDS/named-pipe server, wiring, CLI entry
  point.

The workspace `members` list is a flat array specifically so unit U13's
`rust_core/agent` and `rust_core/coordinator` are one-line additions, not a
restructuring.

**Naming collision:** the package is literally named `core`, colliding with
Rust's own sysroot `core` crate. The on-disk directory and `[package] name`
stay `core` to match TRS §3 verbatim; any future dependent imports it under
an explicit alias instead:

```toml
ai_ops_core = { package = "core", path = "../core" }
```

so application code never has to write an ambiguous bare `core::…` path.
`core` has zero consumers in unit U0, so this is documented now (in
`rust_core/core/src/lib.rs`) and exercised for real starting unit U1.

## Consequences

- A change that moves logic across one of these four boundaries (e.g. moving
  something from `service` into `core`) is a normal code change; a change
  that adds a *fifth* v1.x crate, renames one of the four, or moves the
  `unsafe`/`Command` permission elsewhere requires a new ADR.
- `platform`'s crates carry their own local `[lints]` table rather than
  inheriting the workspace's `unsafe_code = "forbid"` (see ADR 0005) — this is
  the one place in the workspace lint configuration that is not simply
  `[lints] workspace = true`.
