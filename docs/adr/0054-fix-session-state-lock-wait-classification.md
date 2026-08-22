# 0054 — Core: Fix Session-State Lock-Wait Classification

## Status

Accepted (unit U49).

## Context

ADR 0026 (U19, demo scenario harness) and ADR 0036 (U31, DBMS
sessions/locks endpoints) each independently discovered — and
explicitly flagged, but did not fix, per their own FORBIDDEN scope — a
real bug in `core::dbms::adapters::postgresql::queries::
classify_session_state`:

```rust
fn classify_session_state(state: Option<&str>, wait_event_type: Option<&str>) -> SessionState {
    if wait_event_type.is_some() {
        return SessionState::Waiting;
    }
    ...
}
```

Real PostgreSQL sets `wait_event_type` for *any* backend wait, not only
a lock wait — `'Client'` (ordinary idle-on-socket wait between
commands, including a session idle inside an open transaction), plus
`'IO'`, `'Timeout'`, `'Extension'`, `'IPC'`, etc. Only `'Lock'` means
"blocked waiting for another session's lock." Treating any non-null
value as `Waiting` meant a genuinely idle-in-transaction backend
classified as `WAITING`, not `IDLE_IN_TRANSACTION` — confirmed still
true by direct read before this unit (unmodified since both ADRs), and
confirmed no later ADR had closed it.

This was a live, load-bearing scoring bug, not cosmetic:
`core::analysis::db::lock_waits` feeds the `DB_LOCKS` root cause the
correlation engine ranks by counting `SessionState::Waiting` sessions
directly. ADR 0026 documented this inflating `DB_LOCKS` confidence
during a demo scenario purely from ordinary idle client connections — a
real false-positive source in the product's own correlation output.

## Decision

Narrowed the check to `wait_event_type == Some("Lock")` specifically —
the literal PostgreSQL value already used verbatim elsewhere in this
codebase's own tests to poll for a genuine lock wait. Every other
non-null `wait_event_type` now falls through to the existing
`state`-column-based classification unchanged (`active` → `Active`,
`idle in transaction` (+ aborted) → `IdleInTransaction`, everything
else → `Idle`) — matching PostgreSQL's own real semantics:
`wait_event_type` and `state` are orthogonal columns, and only a
`'Lock'` wait means genuine lock contention.

`classify_session_state` has exactly two call sites — `list_sessions`
(`GET /api/v1/dbms/sessions`, and the field `lock_waits` reads) and
`long_transactions` (`GET /api/v1/dbms/long_transactions`) — both fixed
by the one shared-function change.

**No wire/schema/console impact**, confirmed by direct read before
implementing: `contracts::SessionState` and `service::api::v1::dbms::
to_wire_state` already mirror the same 4 variants
(`Active`/`Idle`/`IdleInTransaction`/`Waiting`) 1:1 — only which real
PostgreSQL states map to `Waiting` changed, not the variant set. No
`contracts` change, confirmed via a clean `check-schema-drift.sh` run.
No console screen needed touching.

## Verification (real, not simulated)

`service/tests/dbms_endpoints.rs`, both real `TestPostgres`-backed:

- `sessions_returns_a_real_idle_in_transaction_backend_correctly`
  (renamed from `sessions_returns_a_real_idle_backend_as_waiting`,
  whose own doc comment used to narrate the bug): a real backend with
  an open `BEGIN;` and no further query now correctly reports
  `IDLE_IN_TRANSACTION` via `/api/v1/dbms/sessions`, not `WAITING`.
- `locks_returns_a_real_blocking_edge_from_genuinely_induced_contention`:
  extended with a second real assertion — the same genuinely blocked
  waiter (`wait_event_type = 'Lock'`, confirmed via a real
  `pg_stat_activity` poll, not a guessed sleep) that already proves the
  `/api/v1/dbms/locks` blocking edge also shows `WAITING` via
  `/api/v1/dbms/sessions`. This is the real contrast that makes the fix
  verifiable: a genuine Lock-wait now classifies distinctly from a
  genuine Client-wait/idle-in-transaction session, not just "the old
  wrong assertion is gone."

Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
clean. Both grep gates clean (pure Rust logic change, zero new SQL).
Schema-drift clean. `cargo deny check`/`cargo audit` clean. No leaked
PostgreSQL processes. Full `cargo test --workspace --test-threads=1`
run clean (0 failures this run, including the previously-noted
`tuning_plan_concurrency_test` race, file untouched by this unit).

## Consequences

- `GET /api/v1/dbms/sessions` and `GET /api/v1/dbms/long_transactions`
  now report `IDLE_IN_TRANSACTION` correctly for a real idle-in-
  transaction session, instead of misreporting it as `WAITING`.
- `core::analysis::db::lock_waits` — and therefore the `DB_LOCKS` root
  cause the correlation engine ranks — no longer counts ordinary idle
  client connections as lock waiters. This directly improves the
  accuracy of a real, already-observed false-positive in the
  correlation engine's own output (ADR 0026's demo-scenario finding).
- Closes the gap both ADR 0026 and ADR 0036 named and deferred.
