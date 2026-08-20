# 0025 — Service + Console: Cross-Layer Correlation Wiring

## Status

Accepted (unit U20).

## Context

The user's real goal was a demo that shows an outside DBA the U5/U12
correlation verdict (`core::correlation::correlate`) live, in under 10
minutes. Research for that demo found the verdict had no path to the
console at all: `correlate()` had never been wired past `core`'s own
tests — no `contracts` wire types, no `service` endpoint, no console
screen. Two of the three intended demo scenarios (lock storm, connection
pool exhaustion) are DB-side conditions, and `correlate()` needs a
`DbHealthVerdict` from `classify_db`, which needs a live `DbmsAdapter`
poll — a gap ADR 0024 (U19) already found and deferred. Per the user's
explicit choice, that work was split out into this unit (U20) before the
demo harness itself (a separate, later unit) can be built on top of it.

`core::dbms` (U4) turned out to be far more complete than ADR 0024's own
framing assumed: a full `DbmsAdapter` trait, a real PostgreSQL
implementation, a real `deadpool_postgres` pool with TLS handling. What
was actually missing was narrower than "DB support" — just the
connection lifecycle inside `service` itself, which had never called any
of it.

## Decision

**Nine `RootCause` variants plus `Hypothesis`/`RuledOut`/
`CorrelationResult`, mapped field-for-field into `contracts::
correlation`, reusing `contracts::analysis::Evidence` rather than
redefining it.** The schema confirms this: `correlation_result.schema.json`
`$ref`s the same `Evidence` definition `host_verdict.schema.json` uses,
not a duplicate.

**`service` builds its own DB connection pool directly, bypassing
`core::dbms`'s `CredentialStore`/OS-keyring path entirely.**
`KeyringCredentialStore` targets a desktop session (Secret Service/
Keychain); a headless `service` process has no such session to draw on.
`service::dbms_config::resolve_db_connection` reads plain env vars
(`ASTEROPS_DB_HOST`/`_PORT`/`_DATABASE`/`_USER`/`_PASSWORD`/`_SSLMODE`,
all-or-nothing on host+password) and hands the result straight to
`dbms::pool::build_pool` + `PostgresAdapter::from_pool` — the same
alternate constructor `core::dbms` already exposed for "a caller that
already has a configured `Pool`." Same category of deliberate,
documented simplification as the "no auth/session system exists yet"
gap ADR 0021 already flagged for console mutations, not a new one.

**A real, non-obvious consequence of that choice, worth stating
explicitly**: `from_pool` skips the least-privilege role check
`PostgresAdapter::connect` normally performs (`role_check::validate`,
which blocks a superuser connection in `Production` unless explicitly
overridden). `service`'s connections made this way are never
role-checked. Acceptable for this unit's scope (a demo/monitoring
connection, not a hardened multi-tenant deployment), but a real gap a
future security-hardening unit should close, not silently inherited.

**All-or-nothing on the DB poll.** `compute_db_verdict` fires the ~10
`DbmsAdapter` calls `DbEvidenceBundle` needs concurrently via `tokio::
try_join!`; if any one fails, the whole bundle becomes `analysis::
unavailable_verdict(reason, now)` rather than a partially-real,
partially-faked bundle. No fabricated defaults for a failed call.

**"No DB configured" and "DB poll failed" are both real, valid `200`
responses, never errors.** `unavailable_verdict` already makes every DB
check `Unavailable`, and `confidence::db_cause_confidence` already
averages `Unavailable` out to `0.0` (confirmed by its own existing unit
tests) — so all four DB-side `RootCause`s land in `ruled_out` with the
existing "no DB evidence available" message, with zero new `core`
logic. `503 Unavailable` stays reserved for "the repository layer never
started," exactly matching U19's own precedent — `correlation_endpoints
.rs`'s `a_refused_db_connection_degrades_to_ruled_out_never_a_503` test
proves this against a real closed TCP port, not a mock.

**Per-request, not a background poll loop** — same shape as U19's
`classify_host` wiring, and `compute_host_verdict` is now a shared
helper both the `host` and `correlation` handlers call, rather than
duplicated. No `dbms_snapshots` history table; DB evidence stays an
inherent single-poll snapshot (`analysis/db.rs`'s own pre-existing doc
comment), a real asymmetry with host evidence's 5-minute window that
this unit doesn't attempt to fix.

## Real discovery while writing the lock-storm integration test

The first version of `a_real_lock_storm_ranks_db_locks_from_genuinely_
induced_contention` held the lock via `LOCK TABLE t IN ACCESS EXCLUSIVE
MODE` — and every run degraded to `unavailable_verdict` instead of
ranking `DB_LOCKS`. The cause: `DbmsAdapter::table_stats` calls
`pg_total_relation_size(...)` for every row in `pg_stat_user_tables`,
including the locked table — and that function needs an
`AccessShareLock` on the relation to read its size, which conflicts
with `ACCESS EXCLUSIVE`. The endpoint's own poll blocked on its own
lock-holder, hit `pool.rs`'s `statement_timeout=5000`, and the whole
bundle failed — not a product bug (a real monitoring tool's own size
query *should* block under a genuine whole-table exclusive lock; that's
correct, expected behavior), but the wrong scenario for this test to
construct. Fixed by holding a real row-level lock instead (`SELECT ...
FOR UPDATE`), which is compatible with `AccessShareLock` and leaves
`table_stats` unaffected — a faithful reproduction of an ordinary row-
contention lock storm, six genuinely blocked waiter sessions confirmed
via a real `pg_stat_activity` poll loop (not a guessed sleep), same
discipline as this session's SIGSTOP/SIGCONT polling precedent.

## Two CI grep gates widened, matching their own existing `core/tests/`
exception exactly

`correlation_endpoints.rs` needs `std::process::Command` (to spawn a
real, throwaway `initdb`/`pg_ctl`) and raw SQL literals (to seed/inspect
that fixture) — the same category of test-only DB-fixture code
`core/tests/dbms/common/mod.rs` already does, which both gates already
exempt. `scripts/check-no-command-outside-exec.sh` and `scripts/check-
no-sql-outside-adapters.sh` now also exempt `rust_core/service/tests/`,
for the identical, already-documented reason: `service` itself still
issues zero raw SQL and zero direct process spawns of its own — every
real DB access goes through `DbmsAdapter`, matching every other
`service` endpoint.

**A deliberate, scoped duplication, not a new shared crate:**
`service/tests/support/mod.rs` is a trimmed copy of `core/tests/dbms/
common/mod.rs`'s `TestPostgres` (start/stop/connect only — no TLS, no
replica support, neither of which this unit's tests need). Cargo
integration tests in different packages can't share code across a
`tests/` directory without a new shared test-support crate, which would
be a bigger, separate structural change than this wiring unit's scope
calls for. A future unit adding a third caller should consider
extracting one.

## Console

Same read-only `PolledRepository`/`StreamProvider.autoDispose` pattern
as `analysis_providers.dart`. `correlation_screen.dart` renders every
field `CorrelationResult` carries — ranked hypotheses (cause,
confidence, evidence) and the ruled-out list (cause, reason) — with a
presentation-only cause→label mapping (`_causeLabel`), same discipline
as `host_analysis_screen.dart`'s bottleneck labels. Fourteenth
`NavigationRail` destination, reconfirmed against the full 32/32
`flutter test` suite.

## Consequences

- `GET /api/v1/analysis/correlation` and its console screen are the
  first real exposure of `core::correlation::correlate` and
  `core::analysis::classify_db` outside their own unit tests.
- `service`'s only DB connections skip `core::dbms`'s least-privilege
  role check (see above) — flagged, not fixed, this unit.
- `core::ai` wiring remains a wholly separate, unstarted candidate.
- The demo harness itself (three no-Docker scenarios, expected-verdict
  checks, RUNBOOK) is a separate, later unit built on top of this one's
  real endpoint and screen — not attempted here.
