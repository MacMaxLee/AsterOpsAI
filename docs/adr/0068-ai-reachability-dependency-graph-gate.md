# 0068 — Correlation-Engine AI-Reachability Dependency-Graph Gate

## Status

Accepted (unit U63).

## Context

TRS §20 states: "No AI call is reachable from this [correlation engine]
code path — verified by a CI dependency-graph check in U15, not just by
review." ADR 0066's audit confirmed this had never been built, alongside
TRS §46's RBAC docs-sync gate (deferred there as genuinely not buildable
until the v2 coordinator exists — that one remains deferred). This one has
no such blocker: `ai_ops_core::correlation` and `ai_ops_core::ai` already
exist as real, separate modules today.

A plain textual grep for `ai::` inside `correlation/*.rs` (the same style
as the existing `check-no-command-outside-exec.sh` / `check-no-sql-
outside-adapters.sh` gates) would satisfy the letter of "a check" but not
TRS's specific word "**dependency-graph**": a transitive dependency routed
through an unrelated third module (say, `correlation` → `analysis` →
`ai`) would be invisible to a grep scoped to `correlation/`'s own files.

## Decision

**`cargo-modules`** (rust-analyzer-backed, compiler-grade name resolution
— not text matching) generates the real per-item dependency graph for the
`core` crate. `scripts/check-no-ai-reachable-from-correlation.sh` runs
`cargo modules dependencies -p core --lib --layout dot`, then does a real
BFS graph-reachability search from every item under
`ai_ops_core::correlation` over **`uses`-labeled edges only** — the tool's
own distinction between an actual code dependency and an `owns` edge
(module-nesting structure, e.g. "`ai_ops_core` owns `ai_ops_core::ai`",
true regardless of whether anything calls into it). Any node under
`ai_ops_core::ai` reachable that way fails the gate.

**No prebuilt binary distribution exists for `cargo-modules`** — checked
directly: its GitHub releases carry no binary assets, and it isn't in
`taiki-e/install-action`'s supported-tool manifest. `.github/workflows/
ci.yml`'s new `ai-reachability` job therefore compiles it from source
(`cargo install cargo-modules --locked --version 0.27.0`, ~2.5 minutes)
but caches the resulting binary keyed on the pinned version, so only the
first run after a version bump pays that cost.

## Verification (real, not simulated)

- **Real current state, clean**: `bash scripts/check-no-ai-reachable-from-
  correlation.sh` against the actual repo passes — no item under
  `ai_ops_core::ai` is reachable from `ai_ops_core::correlation`.
- **Real induced violation, correctly caught — twice.** First attempt (a
  function returning a nonexistent `ai::schema::AiResponse`) exposed a
  real gap in the test methodology, not the gate: the code didn't compile
  (`cargo check` confirmed `E0425`), and rust-analyzer correctly emitted no
  edge to a type that doesn't exist — a false-negative-looking result that
  was actually correct behavior for broken input, not a gate bug. Fixed
  the test to reference a real type (`ai::schema::AiExplanation`) that
  compiles cleanly; the gate then correctly failed, reporting not just the
  direct reference but every type transitively reachable through it
  (`MetricClaim`, `Observation`, `Recommendation`, `RiskLevel` — its own
  struct fields), proving real multi-hop BFS, not a one-edge check. The
  injected function was removed immediately after confirming.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check` clean
  (this unit touched only a new script and the CI workflow — no product
  Rust code changed). `dart format`/`analyze`/`test` unaffected (no
  console changes). No leaked PostgreSQL processes.

## Consequences

- TRS §20's dependency-graph requirement is now real, CI-enforced, and
  demonstrably catches both direct and transitive violations — not merely
  asserted in prose or left to code review.
- The remaining ADR-0066-named backlog (the ~27-FR-ID `NEEDS-VERIFICATION`
  set, TRS §46's RBAC docs-sync gate) is unchanged by this unit and still
  open.
- `cargo-modules`' pinned version (`0.27.0`) is now a real, if narrow, tool
  dependency for this repo's CI, outside the crates already vetted by
  `cargo deny`/`cargo audit` for the product's own dependency graph — a
  dev-tooling dependency, not a shipped one, but worth noting if it's ever
  audited for supply-chain scope.
