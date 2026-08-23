# 0072 — FR-CORR-003 Status Correction: Console Wiring Real, AI Layer Still Missing

## Status

Accepted (unit U66).

## Context

ADR 0069 (the NEEDS-VERIFICATION backlog closure) left FR-CORR-003 open,
citing `core::correlation::mod.rs`'s own module doc: "`CorrelationResult`
stays `core`-internal, not a `contracts` wire type... a future service/
console-wiring unit should promote all three [`HostVerdict`/
`DbHealthVerdict`/`CorrelationResult`] together (see docs/adr/0017)."
That doc comment turned out to be **stale, not current** — checked
directly rather than trusted:

- `rust_core/contracts/src/correlation.rs` already exists, added by unit
  U20 (per its own doc comment), with a full field-mirrored
  `CorrelationResult`/`Hypothesis`/`RuledOut`/`RootCause`.
- `rust_core/service/src/api/v1/analysis.rs` has a real
  `to_wire_correlation` conversion function and a real
  `pub async fn correlation(...)` handler — a genuinely wired
  `/api/v1/analysis/correlation` endpoint, not a stub.
- The console has generated Dart models (`correlation_result.dart` and
  friends) and real l10n strings for ranked hypotheses, ruled-out
  causes, and confidence — real rendering, not scaffolding.

So the "promote all three together" deferral ADR 0017 described was
real, but it already happened, in unit U20, six unit-ADRs after ADR
0017 itself — `core::correlation::mod.rs`'s own doc comment was simply
never updated to say so, and continued to describe a stale precondition
as if it were still true.

**What's genuinely still missing**: FR-CORR-003's exact text names two
consumers — "the console **and** the AI explanation layer." Only the
console half is real. `core::ai::bundle` has `build_host_bundle` and
`build_db_bundle` (each feeding `ai::try_explain` for host/DB analysis
respectively) but no `build_correlation_bundle` — there is no AI
explanation feature for correlation output at all yet. This is a real,
separate, moderately-sized feature (a new bundle-builder, a new explain
endpoint, new console UI to trigger it), not a documentation or test
gap — confirmed with the user, who chose to name it as an honest,
deferred follow-up rather than build it as part of this correction.

## Decision

- `core::correlation::mod.rs`'s FR-CORR-003 doc comment is corrected to
  state the real, current status: console wiring is real and cites
  where; the AI-explanation gap is named explicitly, with a pointer to
  this ADR, rather than repeating the outdated "nothing promoted yet"
  claim.
- No new code beyond the doc comment. Building `build_correlation_bundle`
  and its wiring is explicitly out of scope for this correction — a real
  feature addition for a future unit to pick up, not attempted here.

## Verification (real, not simulated)

- Confirmed directly (not assumed from the stale doc comment) that
  `contracts::correlation`, `service`'s conversion function and
  endpoint, and the console's generated models/l10n strings all
  genuinely exist and are wired end-to-end.
- Confirmed directly that no `build_correlation_bundle`-equivalent
  exists anywhere in `core::ai` — grepped, not inferred.
- No functional code changed; `cargo build/test/clippy/fmt` unaffected
  by this unit (doc-comment-only change).

## Consequences

- A future unit building AI-explains-correlation should start from this
  ADR and `ai::bundle::build_host_bundle`/`build_db_bundle` as the
  direct precedent to mirror (same `try_explain` validator/schema
  pipeline, no new AI-integration pattern needed).
- `docs/traceability-matrix.md`'s FR-CORR-003 row is unaffected by this
  change alone (the module doc comment isn't itself a new grep match
  inside a test file) — it remains `NEEDS-VERIFICATION`, now for the
  correct, narrow, honestly-named reason (AI-layer consumption missing)
  rather than the previous stale claim (nothing promoted).
- This is the second stale-ADR-reference bug found and fixed this
  session (the first: ADR 0069/0071's own `Cargo.toml` comment
  correction) — worth a habit going forward: when an ADR's own "future
  unit should do X" language is cited, check whether a later unit
  already did X before assuming it's still open.
