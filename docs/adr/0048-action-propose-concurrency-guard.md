# 0048 — Core: Concurrency Guard Against Duplicate Action Proposals

## Status

Accepted (unit U43).

## Context

ADR 0034 (U29) named a real gap and deliberately left it open: *"nothing
prevents the same target being suspended twice (proposed and granted
twice) before either is rolled back — no concurrency guard exists for
`/actions/propose` the way `core::tuning`'s partial `UNIQUE` index
guards tuning plans."* The identical gap was independently documented
in `core::repository::policy::find_resumable_action`'s own doc comment.
This unit closes it.

## Decision

**`insert_proposed_action` (`core/src/repository/policy.rs`) gains a
guard**, applied only to fresh proposals (`new.rollback_of.is_none()`
— a rollback attempt's whole point is to resolve an existing
unresolved row, so it must never be blocked by this check; `rollback()`'s
own call site is untouched). Immediately before the `INSERT`, on the
same connection, a `SELECT EXISTS(...)` checks for any existing row
with the same `(action_type, target_identity_json, target_start_time)`
whose status is `AUTO_ALLOWED`/`PENDING_APPROVAL`/`APPROVED`/`EXECUTING`
(always unresolved), or `EXECUTED` with no `ROLLED_BACK` child
(`find_resumable_action`'s own predicate, inlined — that function
itself has no `action_type` filter, so its exact SQL couldn't be
reused directly without changing its own semantics). A match returns
`Err(RepositoryError::ConflictingActionInFlight)` before the row is
ever inserted.

**Why an application-level check instead of `tuning_plans`' own
partial `UNIQUE` index**: that index works because its predicate is a
single flat `status = 'IN_FLIGHT'` — expressible as a SQLite partial
index. This guard's predicate needs a correlated subquery ("no
`ROLLED_BACK` child exists"), which SQLite's partial-index syntax
cannot express. The check-then-insert is still race-free for exactly
the same underlying reason the `UNIQUE` index is: `insert_proposed_action`
runs synchronously on the single writer thread's own connection (ADR
0007) — confirmed by direct read of `repository/writer.rs`'s
`WriteCommand::PolicyPropose` handler, which calls it directly, no
`.await` in between the `SELECT` and the `INSERT`. No other propose
command can interleave between them.

**New `RepositoryError::ConflictingActionInFlight` → `PolicyError::
ConflictingActionInFlight`**, mapped explicitly via a new `map_propose_err`
(`core/src/policy/error.rs`), mirroring the existing `map_transition_err`'s
own discrimination discipline rather than falling through the generic
`Repository(...)` catch-all. `evaluate()` (`core/src/policy/evaluate.rs`)
uses it at its one `propose_action(...).await` call site.

**`service::actions::policy_error_to_api` gets one new explicit arm**:
`PolicyError::ConflictingActionInFlight → ApiError::BadRequest(...)` —
the same `400` category `TuningError::PlanAlreadyInFlight` already
gets (ADR 0028), a real client-correctable condition, not a server
fault.

**Scope is per-`action_type`, not per-target.** ADR 0034 named
"suspended twice" specifically — the same action type proposed twice
against the same target. Two *different* action types both legitimately
in flight against the same target (e.g. a suspend and an unrelated
priority change) are unaffected; `find_resumable_action`'s own query
(unchanged, doc comment corrected) still uses "most recent wins"
across action types for exactly this reason.

## Verification (real, not simulated)

- `core/tests/action_propose_concurrency_test.rs` (NEW, mirroring
  `tuning_plan_concurrency_test.rs`'s own technique): two genuinely
  concurrent `evaluate()` calls (`tokio::join!`) proposing the same
  `Medium`-risk test action type against the same target — exactly one
  succeeds, the other is a real `PolicyError::ConflictingActionInFlight`,
  not silently queued; a `Prohibited`-risk proposal resolves to a real
  terminal `DENIED` immediately, and a second proposal against the same
  target then succeeds, proving resolved states never block.
- `service/tests/action_propose_endpoints.rs` (1 new test): proposing
  `security.suspend_process` twice over real HTTP against the same
  real spawned target — the first genuinely reaches `PENDING_APPROVAL`,
  the second is a real `400` — the exact "double-suspend" scenario ADR
  0034 named, now closed end-to-end.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (all pre-existing `core`/`service` tests, including every
  action-lifecycle and rollback test, still pass unchanged — confirms
  the guard doesn't disturb any already-correct path); both grep gates
  clean (new SQL lives in `core/src/repository/policy.rs`, already
  exempt); schema-drift clean (no wire-contract change — this is a
  pure server-side correctness fix); `cargo deny check`/`cargo audit`
  clean; no leaked processes.

## Consequences

- ADR 0034's named gap is closed: the same target can no longer be
  suspended (or have any other action type proposed) twice before the
  first is resolved.
- `find_resumable_action`'s own doc comment, which previously stated
  this gap as still-open, is corrected to reflect the new guard and to
  explain precisely why "most recent wins" is still the right semantics
  (cross-action-type coexistence, not a residual same-action-type race).
- The double-rollback edge case is unaffected (already independently
  guarded by `rollback_by_row_id`'s own status pre-check plus
  `transition_action`'s CAS) — this unit didn't need to touch it.
