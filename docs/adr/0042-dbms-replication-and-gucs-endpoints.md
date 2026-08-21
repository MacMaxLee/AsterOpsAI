# 0042 — Service: DBMS Replication + Relevant GUCs Endpoints

## Status

Accepted (unit U37).

## Context

ADR 0036/0038/0040 have each deferred the remaining `DbmsAdapter`
methods (`replication_status`, `relevant_gucs`, `temp_file_activity`,
`deadlock_history`, `long_transactions`, `idle_in_transaction_sessions`)
to later slices. `replication_status`/`relevant_gucs` are the next
natural pairing — both cluster-topology/config concerns, distinct from
the activity-monitoring pair (`temp_file_activity`/`deadlock_history`)
and the session-attention pair (`long_transactions`/
`idle_in_transaction_sessions`) left for later units.

## Decision

**New `contracts::dbms::{ReplicationStatus, StandbyInfo, GucValue}`**,
mirroring `core::dbms::{ReplicationStatus, StandbyInfo, GucValue}`
field-for-field.

**`GET /api/v1/dbms/replication`** is the first `/dbms/*` endpoint
whose success payload is a single object, not a `Vec<T>` —
`replication_status()` returns `Result<ReplicationStatus, DbmsError>`
at the `core` level, confirmed by direct read of the `DbmsAdapter`
trait. The 503-vs-200 split is otherwise identical to every prior
endpoint: no DB configured or a genuine poll failure both `503
Unavailable`; success maps straight to the wire type and returns `200
OK`.

**`GET /api/v1/dbms/gucs`** returns `Vec<GucValue>`, the same plain-list
shape as `sessions`/`locks`/`table-stats`/`index-stats`. No
`GatedValue<T>` wrapping for either endpoint — neither method has a
capability-gating concept at the `core` level.

## A real happy path for both, reusing an existing proof

`core/tests/dbms_adapter_smoke_test.rs`'s own fixture already proves
both real happy paths against a real PostgreSQL instance: an ordinary,
non-replica server genuinely reports `{is_primary: true, in_recovery:
false, standbys: []}` (not a special case — every `TestPostgres`
instance is, by construction, a plain standalone primary); and
`relevant_gucs()` genuinely includes `max_connections`. This unit's
own service-level tests reuse that exact real technique, needing no
new fixture. The standby-populated path (a non-empty `standbys` list)
remains genuinely unverified — this project's test harness has no
standby-pair (`TestPostgres` doesn't support streaming replication
setup), a real, separate, much larger future capability, named here
rather than silently assumed to work — the same honest-gap discipline
ADR 0038 already applied to `query_stats`'s untestable `Supported`
path.

## Verification (real, not simulated)

- `service/tests/dbms_endpoints.rs` (4 new tests):
  `replication_is_unavailable_without_a_database` /
  `gucs_is_unavailable_without_a_database` (503, matching every prior
  `/dbms/*` endpoint's own precedent exactly);
  `replication_status_reports_a_real_primary_state` (real `200 OK`
  with `is_primary: true`, `in_recovery: false`, an empty `standbys`
  array) / `gucs_returns_real_settings_including_max_connections` (real
  `200 OK` with a genuine, non-empty `max_connections` setting value).
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (14/14 tests in `dbms_endpoints.rs`, full workspace suite
  green); both grep gates clean; schema-drift clean (`ReplicationStatus`/
  `StandbyInfo`/`GucValue` types + envelopes committed); `cargo deny
  check`/`cargo audit` clean; no leaked PostgreSQL processes.

## Consequences

- Cluster topology (primary/standby, replication lag) and
  operationally-relevant configuration (`max_connections` and friends)
  are now directly queryable over HTTP.
- The real standby-populated path for `replication_status` remains
  genuinely unverified by this project's own test suite — a real,
  pre-existing gap (not introduced by this unit) named explicitly
  rather than silently assumed to work.
- `temp_file_activity`, `deadlock_history`, `long_transactions`, and
  `idle_in_transaction_sessions` all remain real, separate future
  `DbmsAdapter` slices. No console section for replication/GUCs exists
  yet either — real, separate future work, matching the established
  service-then-console pattern.
