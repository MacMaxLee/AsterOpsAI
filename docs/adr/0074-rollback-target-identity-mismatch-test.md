# 0074 — Rollback's Own Target-Identity-Mismatch Refusal, Tested

## Status

Accepted (unit U68).

## Context

ADR 0069 named a real, narrow gap while tagging FR-BENCH-003 as
covered: every rollback test in `actions_rollback_by_row_id_test.rs`
only exercises the success path (identity matches). The forward path
already has this exact test —
`actions_executor_pipeline_test.rs::target_identity_mismatch_aborts_before_apply`
— but nothing proved `rollback()`'s own identical
`verifier.verify(executed.target())?` re-check (its very first line,
before anything is persisted) actually refuses a rollback the same way.

## Decision

New `rollback_by_row_id_refuses_when_target_identity_has_changed` in
`actions_rollback_by_row_id_test.rs`, mirroring the forward-path test's
own shape: execute a real `security.suspend_process` action for real
(the same `execute_for_real` helper every other test in this file
already uses), then call `rollback_by_row_id` with
`&AlwaysMismatchVerifier` (the same test double the forward-path test
uses) in place of the real `ProcessTargetVerifier`. Asserts three real
things, not just the error variant: the call returns
`Err(ActionError::TargetChanged)`; the real process is still stopped
(rollback's own `apply`-equivalent — `resume_process` — never ran); and
the row's own persisted status is still `EXECUTED`, untouched — since
`verify()` fails before the function's own `NewProposedAction` insert,
there's genuinely nothing to roll back to a different state.

## Verification (real, not simulated)

- New test passes; rerun 5× to confirm no timing dependency (real
  process spawn/stop/signal, no async races involved here).
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean.
- `docs/traceability-matrix.md` regenerated: no change — FR-BENCH-003
  was already `OK` via the prior unit's tag on the success-path test;
  this adds real coverage of a documented gap without changing the
  matrix's status for that row.

## Consequences

- Both directions of TRS §27's "re-verified immediately before apply
  *and* immediately before rollback" claim now have a real, symmetric
  test pair — a future reader checking one naturally finds the other.
