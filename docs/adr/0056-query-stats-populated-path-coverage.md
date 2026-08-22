# 0056 — Test Harness: Real `query_stats` `Supported` Coverage

## Status

Accepted (unit U51).

## Context

ADR 0038 (unit U33) named a real, deliberate gap: `query_stats`'s
`Gated<Vec<QueryStat>>` `Supported` (populated) case had never been
proven against a real PostgreSQL instance — only the `Unavailable`
(extension genuinely absent) and no-database paths were tested. The
reason: `pg_stat_statements`, unlike the `pg_monitor`-grantable views
everything else in `core::dbms` reads, requires
`shared_preload_libraries` set *at server start* — not reloadable — to
be loadable at all, and `support::TestPostgres` had no way to pass a
start-only GUC. ADR 0038 explicitly named extending `TestPostgres` for
this as real, separate future work, not attempted then.

Confirmed before implementing: `pg_stat_statements.so`/`.control` are
genuinely present in this sandbox's installed PG17 tree — loadable for
real, not theoretical.

## Decision

**`support::TestPostgres`**: new `start_with_extra_options(major_version,
extra_options: &str)`, appending `extra_options` verbatim to `pg_ctl
start`'s existing `-o` GUC-override string. `start(major_version)`
becomes a thin wrapper (`start_with_extra_options(major_version, "")`)
— every existing call site across every test file keeps working
unchanged; this is additive, not a breaking signature change. A real,
reusable harness capability, not a one-off — any future test needing a
start-only GUC can now use it.

**`service/tests/dbms_endpoints.rs`**: new
`query_stats_returns_real_populated_data`: starts via
`start_with_extra_options(17, "-c
shared_preload_libraries=pg_stat_statements")`, then a real fixture
(`CREATE EXTENSION pg_stat_statements`, a small table, one distinctly-
named parameterized `SELECT` run 3 times) and a real HTTP call, proving
`state == "SUPPORTED"` with the tracked query's real `calls >= 3`.

No `core`/`contracts`/`service` production code changed — the adapter
and endpoint were already correct; this closes a pure test-harness
coverage gap.

## A real discovery while writing the test

The first version's row-finding predicate matched on
`normalized_query.contains("widgets_for_query_stats")` alone — and
found the fixture's own `CREATE TABLE` statement instead of the
intended, repeatedly-run `SELECT`, since `pg_stat_statements` tracks
*every* statement (DDL included), and `.find()` returns the first
match, not necessarily the intended one. The `CREATE TABLE` row had
`calls: 1`, correctly failing the `>= 3` assertion — a real, caught
bug in the test's own selectivity, not a product issue. Fixed by also
requiring `.starts_with("SELECT")`, narrowing to the specific tracked
query.

## Verification (real, not simulated)

- `query_stats_returns_real_populated_data`: real `TestPostgres`
  instance with `pg_stat_statements` genuinely preloaded, real
  extension creation, a real query run 3 times, a real HTTP call
  proving the exact tracked row and its real `calls` count.
- All 23 `dbms_endpoints.rs` tests (1 new) pass; full workspace
  `cargo build/test/clippy -D warnings/fmt --check` clean; both grep
  gates clean; schema-drift clean (no `contracts` change); `cargo deny
  check`/`cargo audit` clean; no leaked PostgreSQL processes.

## Consequences

- `query_stats`'s `Supported` path now has real, end-to-end coverage,
  closing ADR 0038's named gap — `Unavailable`, no-database, and now
  `Supported` are all proven against a real PostgreSQL instance.
- `TestPostgres::start_with_extra_options` is a real, reusable harness
  capability available to any future test needing a start-only GUC —
  not scoped narrowly to this one test.
- ADR 0042's streaming-replication standby-populated gap remains open
  and explicitly deferred — a real, separate, meaningfully larger
  capability (a second full instance, a real `pg_basebackup` step, and
  a novel asynchronous catch-up wait condition), not attempted here.
