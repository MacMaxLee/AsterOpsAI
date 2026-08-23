# 0078 — Dedicated Scheduler for the DB Security/Config Sweep

## Status

Accepted (unit U72).

## Context

ADR 0077 extracted the five best-effort DB-wide security/config checks
(`check_guc_changes`/`check_role_superuser_grants`/`check_role_
membership_grants`/`check_table_privilege_grants`/`check_auth_failures`,
units U55–U60) into `run_security_config_sweep`, but they still
piggybacked on `gucs`'s HTTP handler — the sweep only ran if *something*
happened to poll `/api/v1/dbms/gucs`. ADR 0077 named this larger
question explicitly rather than solving it: "when should it run? on
what interval? does it need its own error/retry posture independent of
the GUC read it currently rides along with?" — confirmed with the user
before starting, since it's a real architecture decision (a new
schedule, a judgment-call interval, a real behavior change for the 32
existing tests that relied on `/gucs` synchronously triggering it), not
a mechanical extraction like U71.

**The real gap this closes**: a headless deployment (or any session
where nobody happens to have the console open polling `/gucs`) would
previously never run *any* of these five security checks at all —
detection was an accidental side effect of unrelated HTTP traffic, not
a real guarantee.

## Decision

**New `service::dbms_security_sweep` module** — the five checks and
`run_security_config_sweep` moved here in full from `api/v1/dbms.rs`,
`run_security_config_sweep` now `pub` (so tests can trigger one sweep
directly and deterministically, the same way `repository::
run_retention_sweep` (unit U2) already is). `spawn`/`spawn_with_interval`
mirror `retention::spawn`'s own shape exactly: `tokio::spawn` +
`tokio::time::interval`, skip-first-tick, loop forever. `spawn_with_
interval` is the test-only override, matching the `self_metrics`
(U62)/`HostTelemetrySampler` (U65) precedent for the same shape.

**`SWEEP_INTERVAL = 30s`** — a real, explicit judgment call, since
neither SRS nor TRS pins a detection-latency target for FR-DBSEC-001.
Frequent enough to catch a superuser grant, a privilege change, or an
auth-failure burst well under a minute; infrequent enough that five real
DB round trips plus a log-file tail don't meaningfully load the
monitored instance. The console's former *accidental* cadence (whatever
its own configurable 1-10s refresh happened to be) was never a
deliberate choice for this.

**`gucs`'s handler is simplified back to exactly what its name says** —
the `if let Some(repo) = ... { run_security_config_sweep(...) }` call is
removed entirely, not just extracted. `main.rs` spawns the new scheduler
once, right after `dbms_adapter` resolves, gated on both a repository
and an adapter being configured (the same precondition the old inline
call always had).

**A real, deliberate test-design decision, not incidental**: the 32
existing tests in `dbms_endpoints.rs` that relied on `GET /gucs`
synchronously running the sweep now call a new `poll_gucs_and_sweep`
test helper instead, which does the real HTTP GET *and* directly invokes
`run_security_config_sweep` once, deterministically — proving the same
detection-pipeline correctness those tests always proved, without now
depending on real background-timer scheduling (which would make them
either flaky or slow). **Scheduling itself** gets its own new, separate
test — `a_real_independent_schedule_detects_a_security_event_without_
ever_polling_gucs` — which never calls `/gucs` at all, proving the
actual point of this unit via `spawn_with_interval` on a short real
interval.

## Verification (real, not simulated)

- All 32 pre-existing `dbms_endpoints.rs` tests pass with the new
  `poll_gucs_and_sweep` helper — same detection assertions, same
  pass/fail outcomes as before this unit.
- New scheduling test passes, rerun 5× to rule out real-timing flakiness
  (uses genuine `tokio::time::sleep`/real wall-clock retry, not
  `tokio::time::pause`, matching this file's own existing real-timing
  precedent in the auth-failure test).
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. Both grep gates and the AI-reachability gate all pass
  (unaffected — this unit touches no `Command`/SQL-literal/AI-boundary
  code).
- `docs/traceability-matrix.md` regenerated: no change — FR-DBSEC-001
  was already `OK` via `core`-level tests untouched by this refactor.

## Consequences

- Security/config detection now runs on a real, independent schedule in
  every deployment, including headless ones with no console ever
  attached — closing the actual gap ADR 0065/0077 both named.
- `gucs`'s handler and doc comment no longer carry any trace of the
  "does more than its name suggests" design debt — a future reader sees
  an accurate, current description.
- `SWEEP_INTERVAL`'s 30s value is a named, documented judgment call, not
  an arbitrary constant — a future unit revisiting detection latency
  should start here, not invent a new number from scratch.
