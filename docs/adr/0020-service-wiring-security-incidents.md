# 0020 — Service Wiring: Security Incidents, the `event_count` Join, and Deferred Incident Closure

## Status

Accepted (unit U15).

## Context

U13 (policy inbox) and U14 (tuning history) established and re-proved
the service-wiring pattern. `repository::security` had the same gap
each of those found in its own domain — real event/insert/check
functions, no list query — so U11's real `security_incidents` rows have
never been visible to anything either. This unit closes it: list open
incidents, and suppress a noisy detector rule.

## Decision

**`list_open_incidents` is a real `LEFT JOIN ... GROUP BY`, not a
denormalized column or a second query per row.** `security_incidents`
has no event-count column; a useful triage list needs one anyway, so the
query joins `security_events` and counts per incident. Ordered
newest-first (`ORDER BY opened_at DESC`) — the same "read backward from
now" reasoning `tuning::list_recent` already used (ADR 0019), distinct
from U13's oldest-first approval inbox: a security triage view reads the
newest alert first, an approval inbox drains from the front. Capped at a
real, explicit `LIMIT 100` — no retention policy exists for
`security_incidents`/`security_events` any more than it did for
`tuning_plans`. `security_endpoints.rs`'s two-events-one-incident case
proves the join for real: a second event correlated into the same
incident (same `detector_id` + resource) moves `event_count` from 1 to
2, not a second incident.

**Listing `WHERE status = 'OPEN'` is the real, correct query even though
it returns the same rows as "list all" today.** ADR 0016 already flagged
that this project has no incident-closing code anywhere — every incident
ever opened stays `OPEN`. That's a fact about today's data, not a reason
to write a looser query; a future unit that adds incident closure
changes nothing about this query's own correctness, because it was
already filtering on the right condition.

**`suppress` needed no new `AppState` wiring — the same registry-free
shape U13's `grant`/`reject` already established.** It only writes
`security_suppressions` and an `audit_events` row through the
`RepositoryHandle` `AppState` already carries; no detector runs inside
`service` yet either (ADR 0016's own scope note — detectors are still
only exercised from `core`'s test suite). `security_endpoints.rs`'s
suppression test proves this endpoint reaches all the way through
`record_event`'s own real suppression check, not merely that a row
landed in `security_suppressions` — a real, end-to-end assertion, not a
shallow one. A second case in the same test proves a resource-scoped
suppression doesn't leak into suppressing a different resource under the
same detector.

**No incident-close/dismiss endpoint — real, separate, genuinely
un-designed work, not just unwired.** What should close an incident?
Auto-resolution once its triggering signal clears? A manual-only
operator action? ADR 0016 left this open on purpose; this unit doesn't
guess at an answer just to round out the CRUD surface.

**No detector-triggering endpoint.** Detectors
(`detect_superuser_override`/`detect_untrusted_device`) are pure
functions called from `core`'s own real callers — the DBMS connect path,
the device sampler — not something an HTTP client invokes directly. This
unit only ever surfaces what a detector already wrote, never runs one on
demand.

## Consequences

- A future incident-closure unit changes `security_incidents.status`
  for real for the first time; `list_open_incidents`'s `WHERE status =
  'OPEN'` filter already does the right thing once that happens, no
  change needed here.
- A future unit that wires real detectors into `service`'s own poll
  loops would give `POST /security/suppress` its first real, live
  audience — this unit's own value doesn't depend on that happening
  first, since an operator can suppress a rule pre-emptively via `core`'s
  test-proven detector functions regardless of what calls them.
- All three service-wiring domains (policy, tuning, security) now follow
  one identical, three-times-proven pattern. A future domain (if any)
  reuses it without re-deriving the shape.
