# 0040 — Service: DBMS Table + Index Stats Endpoints

## Status

Accepted (unit U35).

## Context

ADR 0036 (U31) and ADR 0038 (U33) deliberately left `table_stats()` and
`index_stats()` — two more real, already-built `DbmsAdapter` methods —
for a later slice. Both return plain `Vec<T>` (`Result<Vec<TableStat>,
DbmsError>` / `Result<Vec<IndexStat>, DbmsError>`, confirmed by direct
read of the `DbmsAdapter` trait), not `Gated<T>` like `query_stats`, so
this unit carries none of ADR 0038's capability-wrapping complexity.
Table/index bloat and vacuum activity are a real DBA concern, and this
is the natural next pairing in the same wiring vein.

## Decision

**New `contracts::dbms::{TableStat, IndexStat}`**, mirroring
`core::dbms::{TableStat, IndexStat}` field-for-field (`TableStat`:
schema, table, seq_scan, idx_scan, n_live_tup, n_dead_tup, last_vacuum,
last_autovacuum, total_size_bytes; `IndexStat`: schema, table, index,
idx_scan, size_bytes) — the same deliberately-parallel-not-re-exported
pattern every other `contracts::dbms` type already uses.

**`GET /api/v1/dbms/table-stats`/`GET /api/v1/dbms/index-stats`**
preserve the exact split established by `sessions`/`locks` (U31): no DB
configured, or a genuine `DbmsError` from the poll itself, both map to
a real `503 Unavailable`; a successful poll's real `Vec<T>` is mapped
straight to the wire type and returned as `200 OK`. No `GatedValue<T>`
wrapping — these two methods have no capability-gating concept at the
`core` level to begin with.

## A real happy path this unit can prove (unlike U33)

`query_stats` (ADR 0038) could only prove its real `Unavailable`
degraded path, because `support::TestPostgres` doesn't configure
`shared_preload_libraries` and `pg_stat_statements` is never actually
loadable in this project's test harness. `table_stats`/`index_stats`
carry no such constraint: `core/tests/dbms_adapter_smoke_test.rs`
already proves both return real, populated data against a real
PostgreSQL instance using a plain `CREATE TABLE widgets ...; CREATE
INDEX idx_widgets_name ON widgets (name);` fixture. This unit's
service-level tests reuse that exact real fixture (not a new one) to
prove the same real happy path end-to-end over real HTTP — a genuine
improvement in verification depth over U33's necessarily-degraded-only
proof.

## Verification (real, not simulated)

- `service/tests/dbms_endpoints.rs` (4 new tests):
  `table_stats_is_unavailable_without_a_database` /
  `index_stats_is_unavailable_without_a_database` (503, matching
  `sessions`/`locks`/`query_stats`'s own established precedent
  exactly); `table_stats_returns_real_populated_data` /
  `index_stats_returns_real_populated_data` — real
  `TestPostgres`-backed tests, reusing `dbms_adapter_smoke_test.rs`'s
  own proven `widgets` table + `idx_widgets_name` index fixture,
  asserting genuine populated rows in the real HTTP JSON response
  (`schema == "public"`, `n_live_tup >= 3`, the real index's `table`
  and `schema` fields).
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (10/10 tests in `dbms_endpoints.rs`, full workspace suite
  green); both grep gates clean; schema-drift clean (`TableStat`/
  `IndexStat` types + envelopes committed); `cargo deny check`/`cargo
  audit` clean; no leaked PostgreSQL processes.

## Consequences

- Vacuum/bloat monitoring (dead tuple ratios, unused indexes, table and
  index sizes) is now directly queryable over HTTP, not only folded
  into `analysis.rs`'s own ranked correlation verdict.
- Replication status, relevant GUCs, temp-file activity, deadlock
  history, long transactions, and idle-in-transaction sessions remain
  real, separate future `DbmsAdapter` slices. No console section for
  table/index stats exists yet either — real, separate future work,
  matching the established service-then-console pattern (`DatabaseScreen`
  already exists as the natural home for it).
