# 0052 — Console: DB Analysis AI Explanation UI

## Status

Accepted (unit U47).

## Context

U46/ADR 0051 shipped `GET /api/v1/analysis/db/explain` but stayed
service-only. This is the last console gap in the entire `core::ai`
wiring vein — both AI explanation endpoints (host + DB) now have a
real console surface, matching U45's own host-side precedent (ADR
0050).

## Decision

**Home is `database_screen.dart`, not a new screen.** There is no
plain `GET /api/v1/analysis/db` endpoint anywhere in this service
exposing a raw `DbHealthVerdict` — `/analysis/db/explain` and the
DB-side of `/analysis/correlation` are its only two consumers. Unlike
`host_analysis_screen.dart` (which already displays the verdict this
button sits beside), there's no natural "DB health" screen to attach
to. `database_screen.dart` — the real "Database" nav entry — is the
correct home instead: the explanation is a genuinely independent
on-demand action, not dependent on any verdict already being on
screen.

**Mirrors U45's `_ExplanationSection` exactly** (stage enum,
`ref.read(apiClientProvider)`, the same `GatedValue<T>` vocabulary
reuse, the same deliberately-on-demand-not-polled reasoning). Dart
privacy is per-file, so `_ExplanationSection`/`_GatedExplanation`/
`_ExplanationContent`/`_failureMessage` are reimplemented locally
rather than imported — no cross-file private-widget sharing exists
anywhere else in this codebase either. `_GatedMessage`, however,
collided with `database_screen.dart`'s own pre-existing
`_QueryStatsSection`-only `_GatedMessage` (U34) — renamed to
`_ExplainGatedMessage` for this section specifically, a real name
collision within the same file `flutter analyze` caught immediately,
not a design decision.

**No `zoneHeight` `SizedBox` wrapper** — every zone above it needs one
only because each contains an internal `ListView.builder` requiring a
bounded height (ADR 0047). This section has no internal list, so it's
appended as a plain, unconstrained final child of the existing
`Column`.

**No new l10n keys.** `analysisExplainButton`/`analysisExplanationHeading`/
`analysisExplanationRiskAndConfidence`/`metricState*Title` are all
already subject-generic wording from U45 — reused verbatim.

## Two real testing gotchas found and fixed

1. **The button was genuinely off-screen.** `database_screen.dart`'s
   own six existing zones (each 240px) plus this new section exceed
   an 800×600 test viewport — `tester.tap()` doesn't auto-scroll a
   target into view the way a real user's scroll gesture would.
   `await tester.ensureVisible(find.text('Explain with AI'))` before
   tapping fixes it. This is the *same class* of issue ADR 0047
   already fixed for production layout (a `SingleChildScrollView`
   that scrolls); the fix here is purely on the test side, since
   `SingleChildScrollView` already makes the button reachable — a
   test just has to scroll to it, unlike a production tap gesture.
2. **A generic failure-text collision across sibling zones.** `database_screen.dart`
   watches eleven independently-polled providers; a test that only
   queues 2-3 of them (matching this project's own long-standing
   precedent — every prior zone addition since U34 does this) leaves
   the rest genuinely unscripted, and `AsyncResultView`'s own
   `_failureFor` maps *any* `ApiFailureUnavailable` to the exact same
   generic wording regardless of message — so a test asserting on
   that literal text after inducing a transport failure in the new
   section found **nine** matching widgets, not one, once enough
   sibling zones were also (correctly, honestly) showing their own
   real unscripted-failure state. Fixed with a `Key('dbExplanationError')`
   on this section's own error `Text`, letting the test target it
   precisely via `find.byWidgetPredicate` instead of `find.text(...)`
   — a small, real, justified test-ability addition to production
   code, not a workaround.

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (3 new tests): tapping
  "Explain with AI" renders a real `Supported` explanation
  (summary/observations/recommendations/risk/confidence verbatim); a
  real `Unavailable` gated response shows the real reason through the
  shared vocabulary; a real transport-level error shows the real
  failure message (via the new `Key`) with a working retry that
  recovers into the real explanation.
- All 13 pre-existing `DatabaseScreen` tests re-run and pass
  unchanged — the new section never fires its request until its own
  button is tapped.
- `flutter analyze`/`flutter test` clean (63 console tests total, 3
  new); console codegen-drift clean (no generated-model changes —
  U45's types reused verbatim); no new `.arb` keys; every user-visible
  label routed through l10n (the AI's own prose rendered verbatim,
  same precedent as U45).

## Consequences

- **This completes the console side of the entire `core::ai` wiring
  vein.** Both `build_host_bundle`/`build_db_bundle` explanations are
  now reachable end-to-end: core → service → console.
- `database_screen.dart` now has two differently-named `_GatedMessage`-
  shaped widgets in the same file (`_GatedMessage` for query stats,
  `_ExplainGatedMessage` for the AI explanation) — a real, minor
  naming wart from growing one file across many units, named here
  rather than silently left unexplained; not worth a refactor on its
  own.
