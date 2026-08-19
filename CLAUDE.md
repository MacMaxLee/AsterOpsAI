# CLAUDE.md — AsterOpsAI

Read this file first, every session, before planning or touching code.

## What this project is

AsterOpsAI is a host + PostgreSQL observability tool: a Rust core service,
a thin console client, and (v2) a fleet coordinator. It answers "is
something wrong?" and "is it the database, or is it the box?" with evidence,
not guesses — and, where approved, can change a small number of tunable
settings and verify whether the change helped.

- `docs/SRS.md` — *what* the product must do (functional requirements, FR-IDs).
- `docs/TRS.md` — *how* it's built (technical requirements, referenced by section).
- Precedence rule: SRS wins on *what*, TRS wins on *how*; an ADR resolves any
  conflict that survives that rule.

## How units work

Development proceeds as a sequence of units (U0, U1, U2, …), each defined by
a GOAL / SCOPE (may create/modify) / FORBIDDEN / REQUIREMENTS / DEFINITION OF
DONE block. When implementing a unit:

- Touch only files inside that unit's SCOPE. If a requirement seems to need
  something in FORBIDDEN, stop and flag it — don't silently expand scope.
- A unit is only done when its own Definition of Done passes, not merely
  "it compiles."
- One unit per branch/PR once git is in use; a unit that's taking far longer
  than expected was probably too big — split it and record why in an ADR.

## Workspace map

Four crates under `rust_core/` (see docs/adr/0003-workspace-layout-four-crates.md);
this layout is not restructured without a new ADR:

- `contracts` — every wire type (serde + schemars). No workspace-internal deps.
- `platform` — `PlatformAdapter` trait + linux/windows/macos modules. The
  **only** crate allowed `unsafe` code.
- `core` — telemetry, persistence, analysis, policy, actions, AI, security.
  Package name is literally `core`; dependents must alias it —
  `ai_ops_core = { package = "core", path = "../core" }` — see
  `rust_core/core/src/lib.rs`.
- `service` — the binary: HTTP-over-UDS/named-pipe server, wiring, CLI.

`platform/*/exec.rs` are the **only** modules allowed to call
`std::process::Command`. Both restrictions (`unsafe`, `Command`) are enforced
by CI grep gates (`scripts/check-no-command-outside-exec.sh`), not just
convention — a violation fails the build, it isn't a style nit.

## Contract-first workflow

Every wire type lives in `contracts`. After changing one:

```
scripts/emit-schemas.sh
```

and commit the regenerated `/schemas/*.schema.json` in the same change. CI's
schema-drift gate (`scripts/check-schema-drift.sh`) rejects any PR where the
committed schemas don't match what `contracts` actually emits.

## Lint discipline

- No `.unwrap()`/`.expect()` outside `#[cfg(test)]` — every crate root
  carries `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]`.
- `unsafe_code` is `forbid` workspace-wide except `platform`, which locally
  `deny`s it instead (Cargo's `forbid` can't be downgraded in a child scope —
  see docs/adr/0005-error-handling-strategy.md).

## CI gates

All required, all merge-blocking: `fmt`, `clippy -D warnings`, `test`,
`cargo deny`, `cargo audit`, schema-drift, the two grep gates (Command, SQL),
and `cargo check` for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`.
Reproduce locally with the scripts in `scripts/` plus the plain `cargo`
commands in `.github/workflows/ci.yml`.

## Session hygiene

- No scratch files inside the tree — use the environment's scratchpad
  directory for anything temporary.
- FR-IDs (SRS) are referenced sparingly in code comments and explicitly in
  the tests that cover them (SRS §36 / TRS §45 traceability expectation).
- A later unit that must deviate from an earlier ADR's decision writes a new
  ADR rather than editing a merged one.
