# 0036 — Service: DBMS Sessions + Locks Endpoints

## Status

Accepted (unit U31).

## Context

A fresh, broad survey of SRS/TRS against what's actually built (not the
policy/action pipeline this session had spent many prior units on)
found a large, real gap: `core::dbms::DbmsAdapter`
(`core/src/dbms/mod.rs`) is a fully-built, 12-method trait — sessions,
query stats, lock graph, table/index stats, replication status,
relevant GUCs, temp-file activity, deadlock history, long transactions,
idle-in-transaction sessions — but confirmed by grep, `state.dbms_
adapter` was consumed in exactly one place anywhere in `service`:
`api/v1/analysis.rs`'s correlation endpoint, which only folds it into a
ranked verdict. No direct DBMS data surface existed at all. The same
"built but unwired" pattern this session already closed twice before
(`core::tuning` in U23, `core::security::response` in U26).

## Decision

**Deliberately the smallest real first slice**: `list_sessions()`/
`lock_graph()` only, not all 12 `DbmsAdapter` methods. Both are already
proven at the `core` level — correlation's own lock-storm detector
already exercises `lock_graph()` for real. `query_stats()` returns
`Gated<Vec<QueryStat>>` (`core::dbms::capability::Gated`), whose own
doc comment explicitly deferred its wire-mapping decision until "an
API layer consuming it" existed — that decision (how to represent
`Gated`'s four states — `Supported`/`Limited`/`Unavailable`/
`PermissionRequired` — on the wire, likely mirroring `contracts::
Capability`'s own existing four-state shape) is real, separate work for
a follow-on unit, not folded into this one. Table/index stats,
replication, GUCs, temp-file/deadlock/long-transaction/idle-in-
transaction views are all real, separate future slices too.

**New `contracts::dbms` module** (the first of its kind): `SessionState`,
`SessionInfo`, `LockEdge` — plain structs mirroring `core::dbms::
{SessionState, SessionInfo, LockEdge}` field-for-field. `contracts` has
no workspace-internal dependencies (CLAUDE.md), so these are
deliberately parallel types, not re-exports — the same discipline every
other `contracts` module already follows.

**`GET /api/v1/dbms/sessions`/`GET /api/v1/dbms/locks`**: no DB
configured, or a genuine live poll failure, both degrade to a real,
honest `ApiError::Unavailable` — the same category `analysis.rs`'s own
`compute_db_verdict` already uses ("no DB evidence" is a real, honest
condition, not a server error), not a `500` for something this endpoint
has no business treating as its own bug.

## A real discovery while writing the tests, not assumed from reading code

The first test attempt assumed a genuinely idle-in-transaction backend
(`BEGIN;` with no further query) would classify as `IDLE_IN_
TRANSACTION`. It failed for real: `classify_session_state`
(`core/src/dbms/adapters/postgresql/queries.rs`, unmodified,
pre-existing `core` logic) treats *any* non-null `wait_event_type` as
`WAITING`, checked before the `state` column at all — and real
PostgreSQL reports `wait_event_type = 'Client'` for *any* backend
sitting idle on the socket waiting for its next command, including one
idle inside an open transaction. So a genuinely idle-in-transaction
session classifies as `WAITING` in practice, not `IDLE_IN_TRANSACTION`
— real, existing, deliberate `core::dbms` behavior (its own doc comment
already said "a session can be `active` and *waiting* at the same
time"), out of this unit's scope to change. The test was corrected to
assert what real PostgreSQL actually does, not the original assumption.

## Verification (real, not simulated)

- `service/tests/dbms_endpoints.rs` (NEW, 4 tests), reusing
  `support::TestPostgres` exactly as `correlation_endpoints.rs` already
  does (real `initdb`/`pg_ctl`, no Docker):
  - Both endpoints return a real `503 Unavailable` with no DB
    configured.
  - `sessions`: a real backend is held open (`BEGIN;`, no further
    query) and the endpoint's real response contains that exact
    backend pid — genuine `pg_stat_activity` data, not a fixture. Its
    real state (`WAITING`, per the discovery above) is asserted as it
    actually is.
  - `locks`: reuses the real lock-holder + real-waiter technique
    `correlation_endpoints.rs`'s own `a_real_lock_storm_...` test
    already established (a row-level `FOR UPDATE` lock plus a genuine
    blocked waiter, polled via `pg_stat_activity` until it registers) —
    the endpoint's real response contains the real blocking/blocked pid
    pair.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean (real SQL literals in the new test file
  are covered by the existing `service/tests/*` exemption); schema-drift
  clean (new `contracts::dbms` types + envelopes committed); `cargo
  deny check`/`cargo audit` clean; no leaked PostgreSQL processes (real
  `TestPostgres::stop()`/`Drop` teardown, the same discipline every
  DB-backed test in this codebase already follows).

## Consequences

- An operator (once a console screen exists — real, separate future
  work) can, for the first time, see raw DB session and lock state
  directly, not only correlation's own filtered summary.
- `query_stats`'s `Gated<T>` wire-mapping remains a real, named,
  separate decision for a future unit — not guessed at or half-solved
  here.
- Table/index stats, replication status, relevant GUCs, temp-file
  activity, deadlock history, long transactions, and idle-in-
  transaction sessions all remain real, unwired capabilities — each a
  legitimate, separate future slice of the same pattern.
