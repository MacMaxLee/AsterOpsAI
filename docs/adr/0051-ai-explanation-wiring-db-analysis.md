# 0051 — Service: AI Explanation Wiring (DB Analysis)

## Status

Accepted (unit U46).

## Context

U44/ADR 0049 shipped `/analysis/host/explain`, explicitly deferring
`build_db_bundle` as separate future work. This unit ships it — the
second and final planned slice of the `core::ai` wiring vein
(`core::ai::bundle` exposes exactly two `build_*_bundle` functions:
host and DB).

## Decision

**`compute_db_verdict` split into two layers**, no wire-visible
behavior change for its existing caller (`correlation`). The new
`compute_db_verdict_and_bundle(state, now) -> Result<(DbHealthVerdict,
DbEvidenceBundle), String>` holds the real 10-way `tokio::try_join!`
poll (unchanged) and returns `Err(reason)` with the exact same two
real reason strings `compute_db_verdict` already used ("no database
connection configured" / `"DB poll failed: {err}"`).
`compute_db_verdict` itself becomes a two-line wrapper: `Ok((verdict,
_)) => verdict`, `Err(reason) => unavailable_verdict(&reason, now)`.
`correlation_endpoints.rs`'s full existing test suite (including its
own "no DB configured" and "poll fails" cases) passes unchanged after
the refactor — confirmed by running it, not assumed from reading the
diff.

**`GET /api/v1/analysis/db/explain`** needs the real
`DbEvidenceBundle` itself, not just the classified verdict —
`build_db_bundle`'s own `db_candidates` reads directly from the
bundle's `long_transactions`/`sessions` fields. When no real bundle
exists (DB unconfigured, or a poll failure), this endpoint returns a
real `503 Unavailable` with the same real reason
`compute_db_verdict_and_bundle` produced — **not** a fabricated empty
`DbEvidenceBundle`. Several of its fields (`ReplicationStatus`,
`TempFileActivity`, `DeadlockInfo`) don't derive `Default`, and
inventing plausible zero values for them would manufacture data that
was never actually polled — the same "no real evidence, no fabricated
200" precedent every `/dbms/*` endpoint (and `explain_host`'s own
AI-provider-absence check) already established. Once both the AI
provider and a real DB bundle exist, `build_db_bundle` + `try_explain`
follow the exact same three-outcome `GatedValue<T>` split
`explain_host` (ADR 0049) already established — no new wire types
needed, `AiExplanation`'s shape doesn't depend on its subject.

## Verification (real, not simulated)

- `service/tests/ai_endpoints.rs` (3 new tests, extending `build_app`
  to also accept an optional `dbms_adapter` and reusing
  `dbms_endpoints.rs`'s own `adapter_for(pg)` pattern, reimplemented
  locally per file per the established test-code-can't-cross-package-
  boundaries constraint): `explain_db_is_unavailable_without_an_ai_
  provider` (503, provider absence checked first); `explain_db_is_
  unavailable_without_a_database` (503, provider configured but no
  `dbms_adapter` — asserts the real, distinct
  `"no database connection configured"` message, not just the status
  code); `explain_db_returns_a_real_supported_explanation_from_a_
  real_provider_and_database_round_trip` — a real `TestPostgres`
  instance *and* the real one-shot TCP double from U44, combined,
  genuinely `200 OK` + `GatedValue::Supported`.
- `correlation_endpoints.rs`'s full pre-existing suite re-run and
  confirmed passing unchanged, proving the `compute_db_verdict`
  refactor is behavior-preserving.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (6/6 in `ai_endpoints.rs`); both grep gates clean;
  schema-drift clean (no new types — confirmed zero diff after
  re-running `emit-schemas`, as expected since `AiExplanation`'s wire
  shape is subject-independent); `cargo deny check`/`cargo audit`
  clean; no leaked PostgreSQL processes.

## Consequences

- `core::ai`'s wiring vein is now complete at the service layer: both
  planned bundle builders (`build_host_bundle`/`build_db_bundle`) have
  a real HTTP endpoint. No `core::ai` capability remains unwired.
- No console UI for `/analysis/db/explain` exists yet — real, separate
  future work, matching the established service-then-console pattern
  (`host_analysis_screen.dart`'s own `_ExplanationSection` from ADR
  0050 is the natural template to extend to a DB-health screen, not
  attempted here).
