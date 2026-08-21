# 0044 — Service: DBMS Temp File Activity + Deadlock History Endpoints

## Status

Accepted (unit U39).

## Context

ADR 0036/0038/0040/0042 have each deferred the remaining `DbmsAdapter`
methods. `temp_file_activity`/`deadlock_history` are the next natural
pairing — both real-time-activity cumulative counters read from
`pg_stat_database`, with an identical shape (`{count-like field,
stats_reset: Option<DateTime<Utc>>}`), distinct from the
already-shipped topology/config pair (`replication_status`/
`relevant_gucs`, ADR 0042) and the still-open session-attention pair
(`long_transactions`/`idle_in_transaction_sessions`).

## Decision

**New `contracts::dbms::{TempFileActivity, DeadlockInfo}`**, mirroring
`core::dbms::{TempFileActivity, DeadlockInfo}` field-for-field.

**`GET /api/v1/dbms/temp-file-activity`**/**`GET /api/v1/dbms/deadlock-history`**
follow the exact `replication`/`gucs` shape (ADR 0042): single-object
success payload, `503 Unavailable` on no DB configured or a genuine
poll failure, `200 OK` + real data on success. No `GatedValue<T>`
wrapping — neither method has a capability-gating concept at the
`core` level.

## Genuinely exercised proofs, not idle-fixture trivia

`core/tests/dbms_adapter_smoke_test.rs`'s own assertions for these two
methods are trivially true on a fresh, idle fixture (`temp.temp_files
>= 0`, `deadlocks.deadlocks == 0` — both real, but never actually
exercised). This unit's service-level tests go further, reusing the
exact two-connection/poll-real-state-not-a-guessed-sleep technique
`locks_returns_a_real_blocking_edge_from_genuinely_induced_contention`
(U31) already established:

- `temp_file_activity_reports_a_real_spill_to_disk`: sets `work_mem =
  '64kB'` on a real session, then fetches `200,000` rows from
  `generate_series(...) ORDER BY random()` — an unavoidable external
  sort at that memory ceiling — and polls `pg_stat_database.temp_files`
  directly until it genuinely increments before hitting the HTTP
  endpoint, asserting real `temp_files >= 1` and `temp_bytes > 0`.
- `deadlock_history_reports_a_real_induced_deadlock`: two real
  connections each lock one of two rows (`SELECT ... FOR UPDATE`),
  then each requests the other's row, completing a real lock cycle.
  `deadlock_timeout` is lowered to `50ms` per session so PostgreSQL's
  own deadlock detector aborts one side quickly. The test polls
  `pg_stat_activity` for the real block (not a sleep) before completing
  the cycle, then polls `pg_stat_database.deadlocks` for the real
  increment before hitting the HTTP endpoint, asserting real `deadlocks
  >= 1`.

Both genuinely exercise the code path they claim to prove, rather than
asserting a value that happens to already be true on an untouched
fixture — the same honesty-about-test-depth discipline ADR 0038/0042
already applied when naming a real gap; here the discipline runs the
other way, closing a real gap the smoke test left open rather than
naming one that remains.

## Verification (real, not simulated)

- `service/tests/dbms_endpoints.rs` (4 new tests):
  `temp_file_activity_is_unavailable_without_a_database` /
  `deadlock_history_is_unavailable_without_a_database` (503, matching
  every prior `/dbms/*` endpoint's own precedent exactly);
  `temp_file_activity_reports_a_real_spill_to_disk` /
  `deadlock_history_reports_a_real_induced_deadlock` (both real,
  genuinely exercised, described above).
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (18/18 tests in `dbms_endpoints.rs`, full workspace suite
  green); both grep gates clean; schema-drift clean (`TempFileActivity`/
  `DeadlockInfo` types + envelopes committed); `cargo deny check`/
  `cargo audit` clean; no leaked PostgreSQL processes.

## Consequences

- Temp-file spill activity and real deadlock history are now directly
  queryable over HTTP, both proven against genuinely induced real
  server behavior, not an idle fixture's default values.
- `long_transactions`/`idle_in_transaction_sessions` remain a real,
  separate future `DbmsAdapter` slice. No console section for temp-file
  activity/deadlock history exists yet either — real, separate future
  work, matching the established service-then-console pattern.
