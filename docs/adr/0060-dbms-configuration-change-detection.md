# 0060 — DBMS Security Detection: Configuration-Change Detector

## Status

Accepted (unit U55).

## Context

SRS §34 FR-DBSEC-001 requires deterministic detection of five DBMS
security-relevant event classes, feeding the same incident model as
§20: (a) repeated authentication failure, (b) newly granted
superuser/role privileges, (c) permission changes, (d) connections
from unusual client addresses, and (e) configuration changes.
Confirmed by a dedicated scoping research pass before planning: this
was entirely unimplemented — only `detect_superuser_override` existed,
and it covers a narrower thing (an override actually *used* against a
Production connection), not general role-grant detection.

**A bigger, more foundational finding from the same research**:
`core::security`'s own module doc already stated *"no `service`/
console wiring happens in this unit — detectors aren't hooked into a
live poll loop"* — and this was still literally true. Neither existing
detector (`detect_superuser_override`, `detect_untrusted_device`) had
ever been called from anywhere in `service/src` before this unit; both
were exercised only by their own tests. **This unit is the first real
production trigger for the entire detection→incident pipeline**, not
just the first DBMS-specific one.

**Tractability per sub-item, confirmed by direct research:**

- **(a) repeated auth failure — not attempted.** PostgreSQL exposes no
  SQL-queryable catalog of failed authentication attempts;
  `pg_stat_activity` only shows sessions that succeeded far enough to
  exist. This needs server-log ingestion (`log_connections`, log-line
  parsing, or an extension like `pgaudit`) — a fundamentally different
  I/O class than `core::dbms`'s SQL-view-only design. A real, separate,
  much larger capability.
- **(b) role-grant** and **(c) permission-change detection — not
  attempted.** Both real and tractable (via `pg_roles`/`pg_auth_
  members` and `information_schema.role_table_grants`/`pg_class.relacl`
  respectively), but each needs its own new catalog query and, for (c)
  especially, a real design decision on grant granularity. Natural
  *second* slices once this unit's pattern is proven, not bundled in.
- **(d) unusual client addresses — not attempted.** Real and directly
  precedented (mirrors `known_devices`/`detect_untrusted_device`
  exactly — `pg_stat_activity.client_addr` is already polled via
  `list_sessions`), but it's its own detector/table, not this unit.
- **(e) configuration changes — this unit.** `relevant_gucs()`
  (`core/src/dbms/adapters/postgresql/queries.rs:286-302`, already
  built and wired since U37) already polls exactly the right curated
  9-GUC list via `pg_settings`. No new SQL. Detecting a change is a
  pure diff-against-a-persisted-prior-snapshot problem on top of an
  already-built, already-wired query — the smallest, most tractable
  slice of the five.

**The poll-cadence prerequisite, resolved without new infrastructure**:
`service` has zero standing DBMS poll/scheduler infrastructure (only
host telemetry, retention, and self-metrics run periodic loops).
Detection piggybacks as a best-effort side effect of the existing `GET
/api/v1/dbms/gucs` handler instead — every real call already fetches
the current GUC list; a real cadence is inherited for free from
whatever already polls this endpoint (the console's own
`DatabaseScreen`), with zero new background-task code.

## Decision

**`core::repository::guc_history::record_guc_value`** (mirrors
`device_trust.rs`'s exact shape): atomically reads the last-known
setting for a GUC name (if any), upserts the current value, and
returns the *previous* value — `None` means a first-ever observation
(seeding the baseline, not a change), the identical "empty history
isn't a real event" precedent `detect_untrusted_device` already
established. New table `known_guc_values` (migrations/V14, mirrors
`known_devices`/V12). New `WriteCommand::RecordGucValue` (mirrors
`RecordDeviceSeen`).

**`core::security::detector::detect_guc_change`**: pure, fires only
when a real previous value differs from the current one. New
`DETECTOR_GUC_CHANGED` const, category `"CONFIGURATION_CHANGE"`,
`Severity::Medium` — a documented judgment call (SRS pins no severity
per detector), the same tier `detect_untrusted_device` uses: worth
flagging for review, not inherently as dangerous as a privilege
escalation.

**Wiring**: `service/src/api/v1/dbms.rs`'s `gucs` handler, after a
successful `relevant_gucs()` poll and only when a repository is
configured, loops over each GUC calling `record_guc_value` then
`detect_guc_change`/`security::record_event` when it fires — entirely
best-effort (logged, never blocks the real GUC data the endpoint
exists to serve).

## Verification (real, not simulated)

- `core/tests/security_detector_guc_change_test.rs` (3 tests, mirrors
  the two existing detector test files' exact shape): `record_guc_
  value`'s durability across a fresh `RepositoryHandle` against the
  same on-disk file; `detect_guc_change`'s pure fire/no-fire rules;
  one real end-to-end proof producing a real, queryable
  `security_events`/`security_incidents` row.
- `core/src/security/detector.rs`'s own unit tests (3 cases, matching
  the file's existing style).
- **The strongest proof**: `service/tests/dbms_endpoints.rs`'s new
  `gucs_polling_a_real_changed_sighup_guc_produces_a_real_security_
  incident` — empirically verified before writing this test (a
  standalone, real, throwaway PostgreSQL instance, direct `psql`
  queries) that `autovacuum`/`checkpoint_timeout`/`max_wal_size` (3 of
  the 9 curated GUCs) are real `context = 'sighup'` settings, and that
  a genuine `ALTER SYSTEM SET autovacuum = off; SELECT pg_reload_
  conf();` immediately changes what `pg_settings.setting` reports on
  *any* connection, new or existing. The test polls `/gucs` once
  (seeds the baseline), makes that exact real change against the real
  `TestPostgres` instance, polls `/gucs` again, then calls the
  already-existing `GET /api/v1/security/incidents` endpoint (via the
  same full `api::router` this file's `build_app` always assembles)
  and asserts a real incident now exists naming `autovacuum` at
  `MEDIUM` severity. Passed on the first real attempt and 3/3 reruns.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `repository_migrations_up_down.rs`'s `EXPECTED_TABLES` list
  updated to include `known_guc_values`. Both grep gates clean (the new
  SQL lives in `core::repository`, already an allowed location — the
  same one `device_trust.rs`'s own SQL already lives in). Schema-drift
  clean (no `contracts` change — no new wire type needed). `cargo deny
  check`/`cargo audit` clean. No leaked PostgreSQL processes.

## Consequences

- SRS FR-DBSEC-001(e) is now real and live: a genuine configuration
  change to any of the 9 curated GUCs is detected and correlated into
  a real, queryable security incident, the next time `/api/v1/dbms/
  gucs` is polled by anything (console or otherwise).
- **This is the first time either pre-existing detector's own
  detection→incident pipeline has ever run in a real, `service`-driven
  code path** — a broader, pre-existing gap this unit incidentally
  starts closing, worth naming explicitly since it wasn't previously
  documented as a live fact anywhere.
- Sub-items (a) repeated-auth-failure, (b) role-grant, (c)
  permission-change, and (d) unusual-client-address detection remain
  real, separate, deferred future slices — FR-DBSEC-001 is not fully
  closed by this unit, only its most tractable fifth.
- No console changes were needed: `/security/incidents`'s existing
  screen already renders any incident any detector produces, generic
  across `detector_id`.
