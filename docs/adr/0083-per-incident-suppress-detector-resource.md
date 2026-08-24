# 0083 — Per-Incident Suppress Carries Its Own `detector_id`/Resource

## Status

Accepted (unit U78).

## Context

ADR 0023's own console-security-incidents unit named this gap
explicitly rather than silently working around it: *"A future unit
wanting a genuine per-incident suppress button would need to widen
`SecurityIncidentSummary` (or add an incident-detail endpoint exposing
its underlying events' `detector_id`/resource) on the Rust side
first — this unit doesn't attempt that."* Confirmed directly:
`SecurityIncidentSummary` carried only `id`/`opened_at`/`closed_at`/
`severity`/`status`/`summary`/`event_count` — an incident's own
`detector_id` and resource were nowhere on the wire, only buried in its
correlated `security_events` rows.

## Decision

**Widen `SecurityIncidentSummary`, not a new incident-detail
endpoint** — the simpler of ADR 0023's own two named options, and
sufficient: `list_open_incidents`'/`close_incident`'s existing `LEFT
JOIN`/lookup against `security_events` already has every event's
`detector_id`/`resource_descriptor_json` in scope, so this is a
one-query change, not new plumbing.

**`MIN(detector_id)`/`MIN(resource_descriptor_json)`, not an
arbitrary pick — because it isn't one.** FR-SEC-002's own correlation
rule (`find_open_incident`) only ever reuses an existing `OPEN`
incident for the *exact same* `(detector_id, resource_descriptor_json)`
pair. That's a structural guarantee, not a hope: every event under one
incident is provably identical on both fields, so aggregating with
`MIN` (or any other deterministic reducer) over the group returns the
one true value, never a lossy or non-deterministic guess.

**A new `repository::IncidentDetail` struct** (`incident` +
`event_count` + `detector_id` + `resource_kind` + `resource_name`),
shared by both `list_open_incidents` (open incidents only) and
`close_incident` (any incident) — replacing their previous, narrower
`(SecurityIncidentRow, i64)` tuple return. `resource_kind`/
`resource_name` are parsed from the stored JSON into plain strings in
`repository`, not by depending on `core::policy::ResourceDescriptor`
directly — matching this module's own established "resource/severity
are opaque strings here" convention (see ADR 0075's `Severity`-as-
string reasoning for the same call).

**Console**: `SecurityIncidentsScreen`'s per-row actions grow from one
(close) to two — a real per-incident suppress button, opening the
existing `_SuppressDialog` pre-filled with the incident's own
`detector_id`/resource and locking those two fields (`readOnly`). The
whole point of a per-incident action is suppressing exactly what this
incident already is; leaving the fields editable would just invite a
typo-prone re-entry of information the console already has. The
standalone "Suppress a detector" button (ADR 0020's own original,
pre-emptive use case — suppressing a rule before it's ever fired as an
incident) is unchanged: its dialog still opens with every field blank
and editable.

## Verification (real, not simulated)

- `core/tests/security_incident_correlation_test.rs`: `close_security_
  incident`'s own test extended to assert `detector_id`/`resource_
  kind`/`resource_name` on the returned `IncidentDetail`.
- `service/tests/security_endpoints.rs` (1 new test, 1 extended): a
  real recorded event's incident summary carries the correct
  `detector_id`/`resource_kind`/`resource_name` over the real HTTP
  envelope; the existing close-endpoint test now also asserts these
  survive a close.
- `console/test/security_incidents_screen_test.dart` (1 new test): tapping
  the per-incident suppress button opens a dialog with the detector_id
  field genuinely pre-filled and genuinely `readOnly` (asserted on the
  real underlying `TextField`, not just "some text appeared"), and
  submitting it posts the real, correct `detector_id`/`resource` body.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. Both grep gates and the AI-reachability gate pass
  (unaffected). Console: `flutter analyze`/`flutter test`/`dart format
  --set-exit-if-changed` all clean.
- `scripts/emit-schemas.sh` regenerated `schemas/security_incident_
  summary.schema.json`/its envelope (the only real drift — a field
  addition, not a breaking change) and `dart run tool/generate_models.
  dart` regenerated the matching Dart model; both committed together
  with the Rust change, satisfying `check-schema-drift.sh`/
  `check-console-codegen-drift.sh`.
- `docs/traceability-matrix.md` regenerated: no FR-ID drift (this gap
  wasn't its own tracked FR-ID).

## Consequences

- ADR 0023's own named gap is closed — a per-incident suppress action
  is now real, not blocked on missing wire data.
- `IncidentDetail` is now the one true shape for "an incident plus its
  shared detector/resource identity," used by both the list and close
  paths — a future incident-related endpoint needing the same
  information has a ready-made type instead of re-deriving it.
- The harder, larger question ADR 0023 also flagged in the same
  breath — incident close/dismiss — was independently closed already
  this session (ADR 0081); this unit had no remaining console-facing
  gap of its own left to name.
