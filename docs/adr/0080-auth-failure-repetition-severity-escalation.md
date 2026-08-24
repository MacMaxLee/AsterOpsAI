# 0080 — Repetition-Based Severity for Authentication Failures

## Status

Accepted (unit U75).

## Context

ADR 0065 named one more open gap for FR-DBSEC-001(a) rather than
silently accepting it: SRS's own wording is "*repeated* authentication
failure," but the detector fired `Severity::High` unconditionally on
every single qualifying log line — "the same 'detect the single event
now, defer counting' narrowing this session applied elsewhere (e.g.
ADR 0064's revocation-tracking gap)," in that ADR's own words.

Confirmed with the user before starting (a real design decision, not a
mechanical fix — the same reasoning ADR 0078's own context section
applied to giving the security sweep a real schedule): three concrete
options were on the table — escalate severity on repetition, suppress
isolated failures entirely, or defer again. The user chose escalation:
keep recording every failure (a single genuine attempt is real signal,
never silently dropped), but only treat a *genuinely repeated* pattern
as HIGH.

## Decision

**Two real, explicit judgment calls** (neither SRS nor TRS §37 pins a
number beyond "thresholds tunable per-rule"): `REPEATED_AUTH_FAILURE_
THRESHOLD = 3` failures for the exact same `(user, connection_from)`
pair within `repeated_auth_failure_window() = 5 minutes`. Below
threshold, the event is still recorded, at `Severity::Medium` — never
dropped, never silently suppressed, just not an immediate HIGH alarm
for what's plausibly a typo. At or past threshold, `Severity::High`,
matching the detector's own prior (and still correct) behavior for a
genuine pattern.

**Real persisted history, not a caller-supplied guess.** `security::
auth_failure_resource_descriptor_json` exposes the exact JSON
`detect_auth_failure` will use as this event's resource key; `dbms_
security_sweep::check_auth_failures` queries `repository::count_
recent_security_events` (a new plain read, mirroring `resource_is_
security_flagged`'s own shape) for that exact key before calling
`detect_auth_failure`, so the count reflects genuinely observed,
durable failures — including ones from a prior service run — not an
in-memory counter reset on every restart.

**Incidents now escalate, not just events.** The severity-escalation
option the user picked explicitly required this: an incident's own
`severity` column used to be fixed forever at whichever event happened
to open it (the same "fixed at open" limitation ADR 0065's own
investigation already found for `summary`). `repository::security::
insert_event` now calls a new `escalate_incident_severity_if_higher`
whenever a new event correlates into an already-`OPEN` incident,
raising (never lowering) the incident's stored severity to match. This
is a general fix, not auth-failure-specific — any detector's own later,
more severe correlated event now surfaces correctly on the incident,
not just the event.

## Verification (real, not simulated)

- `core/src/security/detector.rs` unit tests: an isolated failure
  (`recent_failure_count = 0`) is `Medium`; the threshold-th failure
  escalates to `High`.
- `core/tests/security_detector_auth_failure_test.rs` (1 new test): a
  real loop of `REPEATED_AUTH_FAILURE_THRESHOLD` recorded events,
  counted via the real persisted `count_recent_security_events` path
  (not a hand-supplied count), confirms the incident's own stored
  severity reaches `HIGH`.
- `core/tests/security_incident_correlation_test.rs` (2 new tests): a
  later more-severe event raises a `Medium` incident to `High`; a later
  milder event never lowers a `High` incident back down.
- `service/tests/dbms_endpoints.rs`: the existing single-failure test
  updated (now asserts `MEDIUM`, not `HIGH` — this is the fix's whole
  point) plus one new real end-to-end test against a genuine PostgreSQL
  instance: `REPEATED_AUTH_FAILURE_THRESHOLD` real wrong-password
  attempts against the same role, polled/swept in a retry loop, must
  produce a `HIGH` incident.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. Both grep gates and the AI-reachability gate pass (unaffected
  — pure detection/persistence logic, no `Command`/SQL-literal-outside-
  adapters/AI-boundary changes).
- `docs/traceability-matrix.md` regenerated: FR-DBSEC-001 remains `OK`.

## Consequences

- FR-DBSEC-001(a) no longer has a standing "fires HIGH on literally
  every single failure" mismatch against SRS's own "*repeated*" wording
  — ADR 0065's last named gap for this FR-ID is closed.
- Every detector's incidents now correctly reflect the worst severity
  among their correlated events, not just whichever one opened them —
  a real, generally-applicable correctness fix beyond auth-failure
  alone.
- `REPEATED_AUTH_FAILURE_THRESHOLD`/`repeated_auth_failure_window()`
  are named, documented judgment calls a future unit can retune without
  archaeology, the same precedent `SWEEP_INTERVAL` (ADR 0078) already
  set.
