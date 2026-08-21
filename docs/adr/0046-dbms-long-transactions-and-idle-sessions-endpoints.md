# 0046 — Service: DBMS Long Transactions + Idle-in-Transaction Sessions Endpoints

## Status

Accepted (unit U41).

## Context

The last remaining `DbmsAdapter` pair: `long_transactions`/
`idle_in_transaction_sessions` — both session-attention concerns, the
natural completion of ADR 0036/0038/0040/0042/0044's own wiring vein.

## Decision

**New `contracts::dbms::{LongTransaction, IdleInTransactionSession}`**,
mirroring `core::dbms::{LongTransaction, IdleInTransactionSession}`
field-for-field. `LongTransaction.state` reuses the existing
`contracts::SessionState` enum and `service/src/api/v1/dbms.rs`'s
existing `to_wire_state` mapper — not a new enum, not a new mapping
function.

**`GET /api/v1/dbms/long-transactions`**/**`GET /api/v1/dbms/idle-in-transaction-sessions`**
follow the exact `table-stats`/`index-stats` shape: plain `Vec<T>`
success payload, `503 Unavailable` on no DB configured or a genuine
poll failure, `200 OK` + real data on success. No `GatedValue<T>`
wrapping.

## A real, named test-depth gap — not silently assumed to work

Both `core` queries filter on a hardcoded 60-second threshold
(`LONG_TRANSACTION_THRESHOLD_SECONDS`/`IDLE_IN_TRANSACTION_THRESHOLD_SECONDS`,
both `= 60.0` in `adapters/postgresql/mod.rs`), confirmed by direct
read. Unlike U39's `temp_file_activity`/`deadlock_history` pair — where
a real spill/deadlock could be induced in milliseconds — genuinely
proving either endpoint's *populated* path requires a transaction or
an idle-in-transaction session to persist past 60 real seconds. Baking
that into the permanent, always-run test suite would add a full minute
to every `cargo test` invocation, forever, to cover one unit's tests —
a cost this project's own test-speed discipline doesn't accept.
`core/tests/dbms_adapter_smoke_test.rs` itself only proves the empty
path for exactly this reason (`long_txns.is_empty()`,
`idle_txns.is_empty()`). This unit's service-level tests match that
same honest scope — proven end-to-end over real HTTP against a real
`TestPostgres` instance, not merely asserted from reading code — while
naming the threshold-crossing populated path as a real, untested gap,
matching the precedent ADR 0038 (query stats' untestable `Supported`
path) and ADR 0042 (replication's untestable standby-populated path)
already set.

Making the threshold configurable (so a test could lower it, the way
`deadlock_timeout` was lowered per-session in U39) would be a real
`core` change — explicitly out of scope for this pure-wiring unit; a
real, separate future capability if ever wanted, not attempted here
just to make a test faster.

## Verification (real, not simulated)

- `service/tests/dbms_endpoints.rs` (4 new tests):
  `long_transactions_is_unavailable_without_a_database` /
  `idle_in_transaction_sessions_is_unavailable_without_a_database`
  (503, matching every prior `/dbms/*` endpoint's own precedent
  exactly); `long_transactions_reports_a_real_empty_result` /
  `idle_in_transaction_sessions_reports_a_real_empty_result` (real
  `200 OK` with a genuine empty array against a real, fresh
  `TestPostgres` instance).
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (22/22 tests in `dbms_endpoints.rs`, full workspace suite
  green); both grep gates clean; schema-drift clean (`LongTransaction`/
  `IdleInTransactionSession` types + envelopes committed); `cargo deny
  check`/`cargo audit` clean; no leaked PostgreSQL processes.

## Consequences

- **This completes the full `DbmsAdapter` service-layer wiring vein.**
  All 12 trait methods are now wired to a real HTTP endpoint:
  `sessions`, `locks`, `query_stats`, `table_stats`, `index_stats`,
  `replication`, `gucs`, `temp_file_activity`, `deadlock_history`,
  `long_transactions`, `idle_in_transaction_sessions` (plus
  `version_info`/`list_databases`, already consumed internally by
  `analysis.rs` since earlier units, never needing their own raw
  endpoint). No `DbmsAdapter` method remains unwired.
- The 60-second threshold-crossing populated path for both endpoints
  remains genuinely unverified by this project's own test suite — a
  real, pre-existing gap, named explicitly rather than silently
  assumed to work.
- No console section for long transactions/idle-in-transaction sessions
  exists yet — real, separate future work, matching the established
  service-then-console pattern (`DatabaseScreen` is the natural home).
