# 0022 — Console: Tuning Plan History, No Detail View, and a Real NavigationRail Overflow

## Status

Accepted (unit U17).

## Context

U16 established the console-wiring pattern and closed the codegen gap
for all three pending schemas at once, so `TuningPlanSummary` was
already a real generated Dart model before this unit started. This unit
is the read-only console follow-up for U14's tuning-history endpoint —
simpler than U16, since U14 itself shipped read-only (no mutation to
wire).

## Decision

**`targetIdentityJson`/`candidatesJson` stay raw, unshown text — no
detail view this unit.** ADR 0019 already established why neither field
has a clean structured projection on the Rust side (`TargetIdentity` has
no shared "name" field the way `ResourceDescriptor` does;
`core::tuning::CandidateOutcome` isn't a `contracts` type at all).
FR-CONSOLE-001 reinforces the same conclusion from the console side:
parsing either field client-side to build a nicer display would be
exactly the "business logic — thresholds, scoring, classification" that
requirement forbids. The list row shows only `profile`, `mode`,
`status`, `createdAt`, and `completedAt` — a real, deliberately smaller
surface than a full detail view, not a placeholder for one.

**Real regression found and fixed during verification: `NavigationRail`
overflowed once a tenth destination was added.** U16 added "Policy" (9
destinations); this unit's own "Tuning" made 10, and the rail's natural
height exceeded the test harness's default window — a real layout bug,
not a test artifact (it would reproduce on any sufficiently short real
window too, not just in `flutter test`). Fixed with Flutter's own
documented pattern for a `NavigationRail` with more destinations than
fit: `LayoutBuilder` + `SingleChildScrollView` + `ConstrainedBox` +
`IntrinsicHeight` around the rail, so it scrolls instead of overflowing
or being silently clipped. This was caught by running the *entire*
console test suite after adding the new screen, not just the new test
file in isolation — `accessibility_test.dart`/`connection_states_test.
dart` both go through the full `RootGate` → `AppShell` path and would
have shipped broken otherwise.

**Still a plain read-only `StreamProvider`/`PolledRepository`, no
`postRaw` involved.** U14's endpoint has no mutation; this screen needed
none of U16's newer mutation-provider pattern.

## Consequences

- The `NavigationRail` fix is now real, load-bearing infrastructure for
  every future nav destination — the security-incidents screen (the next
  planned unit) adds an 11th destination and inherits the fix for free.
- A future unit wanting to show `targetIdentityJson`/`candidatesJson` in
  readable form needs to design real wire types for them first (the same
  gap ADR 0019 already named) — this unit doesn't invent an ad hoc
  client-side parse to work around that absence.
- Running the full test suite after every screen addition — not just the
  new file — is now a proven, necessary step; this unit is the first
  concrete case where skipping it would have shipped a real, user-facing
  layout bug.
