# 0085 — Fix a Reliably-Reproducible Flaky Test Under CPU Contention

## Status

Accepted (unit U80).

## Context

This session twice observed `deadlock_history_reports_a_real_induced_
deadlock` fail under `cargo test --workspace --all-features` (full
parallel test-binary execution — dozens of real PostgreSQL instances
contending for CPU) while passing reliably alone. At the time this was
noted and deliberately left alone (out of scope for the units then in
progress, and non-reproducing on demand).

Investigated properly this time by forcing the same class of
contention deliberately: running `cargo test -p service --test
dbms_endpoints` as 4, then 6, concurrent processes. This reliably
reproduced a failure on every run — but not in `deadlock_history`.
The actual failure was `a_real_independent_schedule_detects_a_
security_event_without_ever_polling_gucs` (unit U72's own scheduler-
independence test, ADR 0078), which never once failed on its own but
failed on 4/4 and 6/6 stress runs. `deadlock_history` itself never
failed once across ten total stress runs — the earlier two sightings
most likely coincided with the disk-full condition the previous
session's own work discovered and fixed (`cargo clean` freed 34GB);
without a reproduction under the current, healthy environment, no
change was made to it — guessing at a fix for a non-reproducing
failure would be exactly the kind of unverified change this project's
own philosophy rejects.

## Decision

**Root cause, confirmed by direct reproduction**: the scheduler-
independence test used a 20ms poll-retry interval (25 × 20ms = 500ms
total budget) to wait for a real background `tokio::spawn` task (on a
20ms sweep interval) to detect and record a security event — far
tighter than every other "wait for something async" test in this file,
which use 200ms × 25 (5 seconds). Under real CPU contention from
several other test binaries' own real PostgreSQL instances, the
background sweep task's own scheduling latency alone can exceed 500ms
without the *tested* logic being wrong at all — this was a test-timing
bug, not a scheduler bug.

**Fix**: widened the retry interval to 200ms (matching this file's own
established convention for these tests, e.g. `gucs_polling_a_real_
auth_failure_produces_a_real_security_incident`'s own retry loop), and
widened the one-time "let the baseline sweep run" wait from 60ms to
200ms for the same reason — it too was tighter than the 20ms interval
it was waiting on deserved under contention.

## Verification (real, not simulated)

- Before the fix: 4 concurrent `cargo test -p service --test dbms_
  endpoints` runs → 4/4 failed, always the same test, confirming a real
  reproduction, not a one-off.
- After the fix: 4 concurrent runs → 4/4 passed. Repeated at 6
  concurrent runs → 6/6 passed.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean under normal (non-stressed) execution.
- `docs/traceability-matrix.md` regenerated: no FR-ID drift (a test-
  reliability fix, not a behavior or requirement change).

## Consequences

- CI (which does run the full workspace test suite, with real
  parallelism across test binaries) should no longer intermittently
  fail this test under normal load.
- `deadlock_history_reports_a_real_induced_deadlock`'s own two earlier
  sightings remain unresolved by this ADR — deliberately, since they
  never reproduced under a healthy (non-disk-full) environment. If it
  recurs, the next investigation should treat it as a fresh, separate
  question rather than assuming this fix also covers it.
