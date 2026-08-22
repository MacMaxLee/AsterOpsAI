# 0058 — Test Harness: Configurable Activity-Detection Thresholds

## Status

Accepted (unit U53).

## Context

A fresh, full re-audit of all 57 ADRs (following U49-U52 closing every
gap from the earlier project-wide audit) found ADR 0046's own named gap
still open: `long_transactions`/`idle_in_transaction_sessions` only
had empty-path test coverage. Both `core` queries filter on a
hardcoded 60-second threshold; proving the real populated path would
mean waiting a genuine 60 seconds in the permanent test suite — a cost
ADR 0046 explicitly declined, instead naming exactly the fix: make the
threshold configurable, the same real, session-scoped-override spirit
`deadlock_timeout` was lowered with in unit U39.

Confirmed by direct read before implementing: `queries::
long_transactions`/`queries::idle_in_transaction_sessions`
(`core/src/dbms/adapters/postgresql/queries.rs`) already accepted
`threshold_seconds: f64` as a plain parameter — the SQL itself was
never hardcoded. Only the two call sites inside `PostgresAdapter`'s
trait impl (`core/src/dbms/adapters/postgresql/mod.rs`) hardcoded the
module-level constants. The `DbmsAdapter` trait's own method
signatures take no threshold argument, so this was entirely internal
to `PostgresAdapter` — no `service`-layer or wire-contract involvement
at all.

## Decision

Added two fields to `PostgresAdapter` — `long_transaction_threshold_
seconds`, `idle_in_transaction_threshold_seconds` — both initialized
to the existing constants in `connect`/`from_pool` (zero behavior
change for every real caller). New builder method
`with_activity_thresholds(long_transaction_seconds, idle_in_
transaction_seconds)` overrides them, test-only — no env var, no
`service`-layer plumbing; `service` never calls it. The two trait
methods now read from `self` instead of the module constants.

`service/tests/dbms_endpoints.rs` gained
`adapter_for_with_zero_activity_thresholds`, overriding both
thresholds to `0.0`, plus two new tests, each opening the exact real
fixture U49's `sessions_returns_a_real_idle_in_transaction_backend_
correctly` already established (`BEGIN;`, no further query):
`long_transactions_reports_a_real_long_running_transaction` and
`idle_in_transaction_sessions_reports_a_real_idle_session`. Both
qualify under a `0.0` threshold deterministically, with **zero
sleep/wait**: by the time the HTTP handler's own query runs, some real
(however small) time has already elapsed since `xact_start`/
`state_change`, so `>= 0.0` is always true — no flakiness risk, unlike
waiting a fixed short delay and hoping it's enough. Both tests passed
cleanly on the very first real attempt.

## Verification (real, not simulated)

- Both new tests pass individually and as part of the full 26-test
  `dbms_endpoints.rs` suite (4/4 clean reruns, no flakiness observed
  in this new code).
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean. Both grep gates clean (no new SQL — reuses the existing,
  unchanged query functions). Schema-drift clean (no `contracts`
  change). `cargo deny check`/`cargo audit` clean. No leaked
  PostgreSQL processes.

## Consequences

- `long_transactions`/`idle_in_transaction_sessions`' real populated
  path now has genuine, end-to-end HTTP coverage, closing ADR 0046's
  named gap — the same category of fix as U51 (`query_stats`) and U52
  (`replication_status`), each closing a previously-deferred
  test-coverage-only gap without touching any already-correct
  production code.
- `PostgresAdapter::with_activity_thresholds` is a real, reusable
  test-only capability — any future test needing a fast, deterministic
  proof of either endpoint's populated path can reuse it directly.
- This closes the last remaining item identified in this session's
  fresh, full ADR re-audit.
