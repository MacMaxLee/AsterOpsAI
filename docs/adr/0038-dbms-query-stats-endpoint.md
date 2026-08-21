# 0038 — Service: DBMS Query Stats Endpoint (`Gated<T>` Wire Mapping)

## Status

Accepted (unit U33).

## Context

ADR 0036 (U31) deliberately deferred `query_stats()` from the first
DBMS slice — it returns `Gated<Vec<QueryStat>>`
(`core::dbms::capability::Gated`), and that module's own doc comment
explicitly left the wire-mapping decision for "an API layer consuming
it" to make. This unit makes that decision and ships the endpoint.

## Decision

**New generic wire type `contracts::capability::GatedValue<T>`**:
`Supported { value: T } | Limited { reason: String } | Unavailable {
reason: String } | PermissionRequired { reason: String }` — the exact
same four states `contracts::Capability` already has, with a payload
added to `Supported`. Not a novel shape: `contracts::telemetry::
MetricValue<T>` already establishes the identical "parametric, tagged,
payload-only-on-the-success-variant" pattern this project uses
everywhere a metric might be gated; `GatedValue<T>` applies that same
shape to `Capability`'s own four states instead of `MetricValue`'s
telemetry-specific ones. New `contracts::dbms::QueryStat` mirrors
`core::dbms::QueryStat` field-for-field.

**`GET /api/v1/dbms/query-stats`** preserves the exact split U31
already established for `sessions`/`locks`: no DB configured, or a
genuine `DbmsError` from the poll itself, both still map to a real
`503 Unavailable` — the same category `analysis.rs`'s own `compute_db_
verdict` already uses for "no DB evidence." Once the adapter call
itself succeeds, its real `Gated<Vec<QueryStat>>` result — whichever of
the four states it actually resolved to, even `Unavailable` (e.g.
`pg_stat_statements` genuinely not installed) — is mapped straight
into `GatedValue<Vec<QueryStat>>` and returned as real `200 OK` data.
The meaningful distinction is "is the whole DBMS feature reachable at
all" (503) versus "we polled successfully and got a real
capability-state answer" (200 + payload) — not a distinction this unit
invents, the same split `analysis.rs` already draws between "no DB
configured" and a real, degraded verdict.

## A real, pre-existing test constraint, confirmed by direct read

`support::TestPostgres` doesn't configure `shared_preload_libraries`,
so `pg_stat_statements` is never actually loadable in this test harness
— confirmed by reading `core/tests/dbms_adapter_smoke_test.rs` (its own
comment: *"query_stats: pg_stat_statements isn't loaded via shared_
preload_libraries... same degrade-to-Unavailable path as the dedicated
no-pg_stat_statements test"*) and `core/tests/dbms_no_pg_stat_
statements_test.rs`. **Neither existing `core` test proves the real
`Supported` happy path either** — both only prove this same honest
degraded path. This unit's own service-level test matches that
already-established scope exactly: it proves the real `Unavailable`
path end-to-end over HTTP (asserting the real `reason` names `"pg_stat_
statements"`, the same real technique both `core` tests already use),
not a fabricated `Supported` case this project's own test
infrastructure genuinely can't produce yet. Extending `TestPostgres` to
support `shared_preload_libraries` (a real restart-with-modified-config
capability it doesn't have today) is real, separate future work, named
here, not attempted.

## Verification (real, not simulated)

- `service/tests/dbms_endpoints.rs` (2 new tests):
  `query_stats_is_unavailable_without_a_database` (503, matching
  `sessions`/`locks`'s own established precedent exactly);
  `query_stats_reports_a_real_unavailable_state_as_200_ok` — a real
  `TestPostgres`-backed test proving a genuine `200 OK` with `{"state":
  "UNAVAILABLE", "reason": "..."}`, the real adapter's own reason
  string, not a fixture.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift clean (`GatedValue`/
  `QueryStat` types + envelope committed); `cargo deny check`/`cargo
  audit` clean; no leaked PostgreSQL processes.

## Consequences

- `GatedValue<T>` is now a real, reusable wire pattern for any future
  endpoint wrapping a `core::dbms::capability::Gated<T>` result (or any
  other future gated capability) — established once, not re-invented
  per endpoint.
- The real `Supported` (happy) path for query stats remains genuinely
  unverified by this project's own test suite — a real, pre-existing
  gap (not introduced by this unit) named explicitly rather than
  silently assumed to work.
- Table/index stats, replication status, relevant GUCs, temp-file
  activity, deadlock history, long transactions, and idle-in-
  transaction sessions all remain real, separate future `DbmsAdapter`
  slices. No console section for query stats exists yet either — real,
  separate future work, matching the established service-then-console
  pattern.
