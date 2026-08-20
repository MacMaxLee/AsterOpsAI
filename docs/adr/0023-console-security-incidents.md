# 0023 — Console: Security Incidents, a Standalone Suppress Action

## Status

Accepted (unit U18).

## Context

U16/U17 proved the console-wiring pattern twice — once with a mutation
(policy grant/reject), once read-only (tuning history). This unit is the
third and final application, for U15's security endpoints, completing
console coverage of all three domains U13/U14/U15 wired into `service`.

## Decision

**Suppression is a standalone action, not a per-incident button —
because it structurally can't be anything else.**
`SecurityIncidentSummary` carries `id`, `openedAt`, `closedAt`,
`severity`, `status`, `summary`, `eventCount` — no `detector_id`, no
resource. U15's own scope kept the summary deliberately minimal (an
event *count*, not the events themselves), so there is nothing in this
wire type to build a "suppress this incident's rule" button from without
either inventing client-side correlation data that doesn't exist, or
making a Rust-side change to widen the summary (out of scope for a
console-only unit). The screen instead exposes a small standalone form —
detector ID, optional resource, reason — reachable from the screen's own
action row, not tied to any specific row. This is exactly the real,
useful shape ADR 0020 already described for this endpoint: "an operator
can suppress a rule pre-emptively... regardless of what calls it."

**The resource fields are both-or-neither, enforced before the request
is ever built.** `ResourceDescriptor` on the wire is `{kind, name}` —
there's no meaningful "just a kind" or "just a name." The dialog checks
this itself (`resourceKind.isEmpty != resourceName.isEmpty`) and blocks
submission with an inline message rather than ever sending a half-built
resource object to the server.
`security_incidents_screen_test.dart`'s own test proves this: filling
only the resource kind field leaves `transport.postedRequests` empty —
the client-side check genuinely prevented the request, not just hid an
error after the fact.

**Severity gets an icon/color mapping, not a re-classification.** The
switch in `_SeverityIcon` picks presentation (which icon, which color)
from whatever string the server already sent; it never invents a new
verdict or reorders severities — FR-CONSOLE-001's "no business logic"
line is about not recomputing what counts as CRITICAL vs. HIGH, not
about refusing to have an icon at all.

**Reused, not reinvented, from U16/U17:** the
`_postForSuccess`/`_decodeEnvelopeSuccess` split (`security.suppress`
returns `()` on success, exactly like `grant`/`reject`), the
`'console-operator'` actor convention (no auth/session system exists
yet, ADR 0021's own flagged gap), and the `Scaffold(body: ...)` widget
test wrapper (`ListTile` needs a `Material` ancestor, U16's own
discovery).

## Consequences

- All three domains U13/U14/U15 wired into `service` now have a real
  console screen. A future domain (if any) reuses this exact,
  three-times-proven pattern with no new architecture to invent.
- A future unit wanting a genuine per-incident suppress button would
  need to widen `SecurityIncidentSummary` (or add a incident-detail
  endpoint exposing its underlying events' `detector_id`/resource) on
  the Rust side first — this unit doesn't attempt that.
- Incident close/dismiss is still entirely unbuilt (ADR 0020's own
  deferral, still standing) — every incident this project can currently
  open stays `OPEN` forever, on both the server and the console.
