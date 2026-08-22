# 0065 — DBMS Security Detection: Authentication-Failure Detector

## Status

Accepted (unit U60). **Closes FR-DBSEC-001 in full** — this is the last of
its five sub-items ((a) auth failure; (b) superuser/role-membership grants,
ADR 0061/0063; (c) table permission grants, ADR 0064; (d) unusual client
addresses, ADR 0062; (e) configuration changes, ADR 0060).

## Context

Unlike U55-U59, this sub-item needed a genuine architecture decision before
any mechanical work could start: PostgreSQL exposes **no SQL-queryable
signal for a failed authentication attempt at all** — only its own server
log (with `log_connections=on`) records one. Confirmed by direct research
against a real, disposable instance, not assumed.

Two real options existed, both with a real cost:

- `pg_read_file()` — SQL-reachable, but requires either superuser or the
  `pg_read_server_files` role, breaking the least-privilege monitoring-role
  contract every other detector in this codebase (`core::dbms::role_check`)
  has preserved.
- Local OS-level log-file tailing — preserves least privilege, but only
  works when `service` and the monitored PostgreSQL are co-located
  (same host, shared filesystem permissions).

Per explicit user direction (delegated as "your best judgment" after a
dedicated scoping discussion), this unit chose **local log-file tailing,
scoped to co-located deployments**. The deciding factor: CLAUDE.md's own v1
scope is already single-host (`FR-FLEET-*`, multi-host coordination, is
explicitly marked v2) — "co-located" isn't a crippling restriction, it's
this product's actual v1 deployment model. When `service` and PostgreSQL
aren't co-located, this one detector simply never fires; every other
detector keeps working regardless.

## Empirical findings (confirmed against a real, disposable instance)

- `log_destination=csvlog` + `log_connections=on` + `logging_collector=on`
  (ordinary `-c` GUC overrides) produce a real, well-structured 26-column
  CSV log file.
- A real wrong-password attempt against a role forced onto `scram-sha-256`
  auth (via a `pg_hba.conf` entry — GUC overrides can't touch authentication
  method, a genuinely separate configuration surface) produces a real
  `FATAL` row with `sql_state_code = '28P01'`.
- `sql_state_code` (`28P01` invalid password, `28000` invalid authorization
  specification), not free-text `message` matching, is PostgreSQL's own
  stable, version-independent signal for "never got in."
- `data_directory`/`log_directory`/`log_destination`/`logging_collector` are
  all real, unprivileged-readable `pg_settings` rows — no new grant needed
  to *locate* the log, only to *read* it (done as direct OS file I/O in a
  co-located deployment, never through PostgreSQL).

## Decision

**A new architectural split, not a mechanical continuation of U55-U59's
pattern.** Every `DbmsAdapter` method until now has been a pure SQL query;
that invariant stays intact — the new `DbmsAdapter::auth_failure_log_config`
is *only* the SQL half (reading the four GUCs above, returning
`AuthFailureLogConfig { csv_logging_enabled, log_dir }`). The actual file
tailing (`core::dbms::log_tail::read_new_auth_failures`) is a **deliberately
standalone, non-trait function** — `service` calls it directly, alongside
the adapter, not through it. Forcing it onto the trait would silently imply
every future adapter must support local file access, which isn't true; a
genuinely network-only adapter could never honestly implement it.

**Deduplication is the byte-offset mechanism itself, not detector logic.**
`core::repository::log_tail_offset` persists `(log_file_path, byte_offset)`
per file (`log_tail_offsets`, migration V19); `read_new_auth_failures`
reads only the bytes appended since the last offset, so a given log line is
parsed exactly once. `security::detect_auth_failure` therefore has **no
fire/no-fire branch and never returns `Option`** — unlike every prior
detector in this file, every event it's given is already, inherently, a
real, new one. `Severity::High` (a documented judgment call — a failed auth
attempt could be an attack in progress).

**A new, real, named design debt.** This is the fifth best-effort check
piggybacked onto `GET /api/v1/dbms/gucs`'s handler (U55-U59 already used it
for lack of a better existing poll point). `gucs`'s own doc comment now
names this explicitly: the handler has grown into a de facto "DB-wide
security/config sweep," doing meaningfully more than its name suggests. A
future unit should probably extract a dedicated internal sweep function;
not attempted here.

**New workspace dependency**: `csv = "1"` (MIT/Unlicense dual-licensed, MIT
already in `deny.toml`'s allowlist). Chosen over hand-rolling parsing via
the already-present `regex` crate: PostgreSQL's `message`/`detail`/`query`/
`context` fields can contain arbitrary embedded commas/quotes/newlines a
hand-rolled parser risks getting wrong.

**New `TestPostgres::start_with_pg_hba_prefix`** (test harness only):
mirrors `start_with_extra_options`'s `initdb` step, but prepends a caller-
supplied line to the generated `pg_hba.conf` before starting — needed
because GUC overrides can't force password auth for one specific role,
which the real wrong-password test requires.

## A real bug this unit's own service-level test caught

The first version of `detect_auth_failure` used `connection_from` alone as
its resource key (mirroring `detect_unusual_client_address`'s own choice).
That turned out wrong here: every local-socket connection reports the
identical `connection_from` value (`"[local]"`), and `TestPostgres::
wait_until_ready`'s own `pg_isready` health check — which connects with no
explicit `-U`, so it authenticates as the sandbox's OS user — produces a
real, unrelated `"role ... does not exist"` auth-failure log line before
the test's own induced failure ever happens. Both events shared the same
resource key, so they silently correlated into the *same* incident
(FR-SEC-002's "related events," stretched past what "related" should mean
here) — and since an incident's `summary` is set once, by whichever event
opens it, the test's own deliberately-induced `pwuser` failure never
surfaced in the incident summary at all.

Fixed by combining `user_name` and `connection_from` into one composite
resource key (`"{user}@{connection_from}"`). Two different users failing
from the same address no longer collide; a genuine repeat offender's own
subsequent failures (same user, same address) still correlate as intended.
The service-level test now asserts both that `pwuser`'s own incident exists
*and* that its summary is distinct from the `pg_isready` noise, proving the
fix rather than only the happy path. This is a real, general finding, not a
test-only artifact: any co-located deployment where a health check or
misconfigured client omits an explicit role will produce the same kind of
benign "role does not exist" noise this detector correctly still flags —
the fix ensures it never masks or gets masked by an unrelated user's real
failure.

## Explicitly deferred (named, not silently dropped)

- **Repetition/threshold-based severity.** SRS's own wording is "*repeated*
  authentication failure"; this unit fires on every single qualifying line,
  the same "detect the single event now, defer counting" narrowing this
  session applied elsewhere (e.g. ADR 0064's revocation-tracking gap).
  Escalating severity or suppressing genuinely isolated failures is real,
  separate future work.
- **Remote (non-co-located) deployments.** Named, not silently assumed to
  work — `pg_read_file()` remains the only SQL-reachable alternative, and
  it was explicitly rejected here for breaking least privilege.
- **Log rotation edge cases beyond "most recently modified `*.csv` file."**
  Correct for PostgreSQL's own normal rotation behavior; a manually deleted
  or externally rotated log directory mid-tail isn't specially handled
  beyond "starts over at byte 0 for whatever file is now active."

## Verification (real, not simulated)

- `core/src/security/detector.rs`'s own unit tests (13 cases total, 3 new:
  severity/summary construction, the collision-fix regression test using
  the exact `parallels`/`pwuser` scenario the real bug exhibited, and the
  graceful-degradation fallback).
- `core/tests/security_detector_auth_failure_test.rs` (4 tests): a real CSV
  fixture file on disk (not a synthetic in-memory value), `log_tail::
  read_new_auth_failures`'s real file I/O and `sql_state_code` filtering
  (proving a `LOG`-level connection row is excluded and a `FATAL`/`28P01`
  row isn't), the byte-offset re-tail proof (re-tailing from the persisted
  offset finds nothing new even though the file still exists), and one
  real end-to-end `security_events`/`security_incidents` proof.
- `service/tests/dbms_endpoints.rs`'s new
  `gucs_polling_a_real_auth_failure_produces_a_real_security_incident`: a
  real `TestPostgres::start_with_pg_hba_prefix` instance, a real `CREATE
  ROLE pwuser LOGIN PASSWORD ...`, a real deliberately-wrong-password
  `tokio_postgres` connection attempt (asserted to genuinely fail), a short
  retry loop polling `/gucs` (the logging collector flushes
  asynchronously), and a real `HIGH`-severity incident found via `GET
  /api/v1/security/incidents` — plus the collision-fix regression
  assertion described above. Found the real resource-collision bug on its
  first run; passes clean after the fix.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check` clean
  (74/74 test binaries, 0 failures). Both grep gates clean. Schema-drift
  clean (no `contracts` change). `cargo deny check`/`cargo audit` clean —
  `csv`'s resolved license lands in the existing MIT/Apache-2.0 allowlist.
  `repository_migrations_up_down.rs`'s `EXPECTED_TABLES` updated with
  `log_tail_offsets`. No leaked PostgreSQL processes. One pre-existing,
  unrelated flaky test observed during the full-workspace run
  (`tuning_plan_concurrency_test::a_second_concurrent_plan_for_the_same_
  target_is_rejected_not_queued`, a concurrency race under parallel test
  load, file untouched by this unit) — reran clean 3/3 in isolation,
  confirmed pre-existing and unrelated.

## Consequences

- SRS FR-DBSEC-001 is now fully covered end-to-end: configuration changes,
  superuser/role-membership grants, unusual client addresses, table
  permission grants, and authentication failures all feed the same
  deterministic detector → incident pipeline (SRS §20).
- This detector is real but deployment-scoped: it only ever fires for a
  co-located PostgreSQL instance whose log directory `service`'s own OS
  process can read. A remote-DB deployment gets every other FR-DBSEC-001
  detector, but not this one — a real, named limitation, not a silent gap.
- `gucs`'s handler is now five concerns deep — a real, acknowledged
  candidate for a future dedicated refactor, not addressed in this unit.
- The resource-key collision this unit's own testing caught is a durable
  lesson for any future detector reusing "address alone" as a correlation
  key: it's only safe when the address itself, not who's behind it, is the
  actually-interesting identity (as it is for `detect_unusual_client_
  address`) — not a general pattern to copy without checking.
