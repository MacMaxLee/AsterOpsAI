# 0081 — Manual Security Incident Close/Dismiss

## Status

Accepted (unit U76).

## Context

Three ADRs named the same gap without closing it. ADR 0016 (unit U11):
*"`security_incidents.status` is `OPEN`/`CLOSED` only — this unit never
writes `CLOSED`... A future unit owning incident resolution/closure is
real, separate work."* ADR 0020 (unit U15): *"No incident-close/dismiss
endpoint — real, separate, genuinely un-designed work, not just
unwired. What should close an incident? Auto-resolution once its
triggering signal clears? A manual-only operator action? ... this unit
doesn't guess at an answer."* `service/src/api/v1/security.rs`'s own
module doc still said the same thing verbatim.

Confirmed directly, not assumed: grep for `"CLOSED"` across
`rust_core/` matched only the doc comments above and the migration's
own column definition — no code path had ever written it.
`contracts::SecurityIncidentSummary` already carries a `closed_at:
Option<DateTime<Utc>>` field, sitting unused since it was added,
visibly anticipating this.

Confirmed with the user before starting: two real options existed
(auto-resolve on signal-clear vs. manual-only operator action), and
they're not close in scope — auto-resolution would need each of the
8 existing detectors to define its own "is this still true?" check,
several of which (a one-time grant event, an auth-failure line) have
no natural "cleared" state at all. The user chose manual-only.

## Decision

**`POST /api/v1/security/incidents/{id}/close`**, mirroring the
existing `/security/suppress` endpoint's shape (`Path<i64>` + a small
JSON body, `Json<CloseIncidentRequest { closed_by }>`). Idempotent:
closing an already-`CLOSED` incident is a harmless no-op that returns
the incident unchanged, not an error — matching this codebase's own
established idempotency precedent elsewhere (e.g. suppression). A
nonexistent id returns `ApiError::NotFound` (404).

**`repository::close_incident`** (mirrors `list_open_incidents`'s own
shape: returns `(SecurityIncidentRow, i64 event_count)` so the API
layer builds a `SecurityIncidentSummary` the same way both endpoints
already do) sets `status = 'CLOSED'`/`closed_at = now` only when the
row is currently `OPEN`, going through the single writer thread (ADR
0007) like every other mutation.

**No `closed_by` column added.** `security_incidents` has no actor
column (unlike suppression's own `created_by`), and adding one for
this alone would be schema churn for a single field. Who closed an
incident is recorded the same way `policy::grant`'s own auto-allowed
path already records its actor: a real `security.incident_closed`
`audit_events` row, not a new column.

**Console**: a per-row close `IconButton` on `SecurityIncidentsScreen`
— unlike the per-row suppress button ADR 0023 explicitly ruled out
(no `detector_id`/resource on the summary to build one from), close
only needs the `id` every summary already carries, so no incident-
detail endpoint was needed to unblock it. Relies on the existing
polling refresh (same as grant/reject/suppress, none of which
explicitly invalidate their provider) rather than an explicit refetch.

## Verification (real, not simulated)

- `core/tests/security_incident_correlation_test.rs` (2 new tests):
  `close_security_incident` sets a real `closed_at`; an unknown id
  returns `None`, not an error.
- `service/tests/security_endpoints.rs` (3 new tests): closing an open
  incident removes it from `GET /security/incidents`' own list and
  leaves a real `security.incident_closed` audit event; re-closing an
  already-closed incident is idempotent (doesn't overwrite the
  original `closed_at`); closing an unknown id is a real 404.
- `console/test/security_incidents_screen_test.dart` (1 new test): a
  real `FakeTransport`-scripted close request; the POST body and the
  success snackbar are both asserted, not just "it didn't crash."
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. Both grep gates and the AI-reachability gate pass
  (unaffected). Console: `flutter analyze`/`flutter test`/`dart
  format --set-exit-if-changed` all clean; `check-console-codegen-
  drift.sh` shows no drift (no wire-schema change — `contracts::
  SecurityIncidentSummary` already had every field this unit needed).
- `docs/traceability-matrix.md` regenerated: no FR-ID drift (this gap
  wasn't its own tracked FR-ID; SRS §20/34's FR-SEC-*/FR-DBSEC-001
  entries are unaffected by adding a closure path).

## Consequences

- Incidents can finally leave `OPEN` — the "every incident stays open
  and keeps its resource flagged indefinitely" conservative default
  ADR 0016 chose deliberately is now something an operator can act on,
  not a permanent dead end.
- The harder auto-resolution question (which ADR 0020 explicitly
  declined to guess at) remains open by choice, not oversight — a
  future unit proposing per-detector "signal cleared" semantics starts
  here, not from scratch.
- `security_incidents` still has no actor column; any future feature
  that needs to query "who closed this" structurally (not just via the
  audit log) is real, separate schema work.
