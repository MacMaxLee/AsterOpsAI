# 0047 — Console: Long Transactions + Idle-in-Transaction Sessions UI

## Status

Accepted (unit U42).

## Context

U41 shipped `GET /api/v1/dbms/long-transactions`/`/dbms/idle-in-transaction-sessions`
(ADR 0046) but stayed service-only, explicitly naming the console side
as future work. This is the **last** console gap in the whole DBMS
wiring vein — once shipped, `DatabaseScreen` fully mirrors every wired
`DbmsAdapter` endpoint (core → service → console, end to end).

## Decision

**Layout**: `DatabaseScreen` gains a sixth (final) vertical zone —
`Row(Long Transactions | Idle in Transaction)` below `Row(Temp File
Activity | Deadlock History)`. Both are plain lists, reusing the
`_TableStatsList`/`_IndexStatsList` shape from ADR 0041, not the
single-object shape from ADR 0043/0045.

**`LongTransaction` rows reuse `_SessionRow`'s existing conventions**
rather than inventing new ones: `state.wireValue` for the
`SessionState` enum, `username ?? '?'` for the optional field, the
`[...].join('  •  ')` subtitle-assembly style, and including the query
text only `if (query != null)` — `LongTransaction` is structurally
close enough to `SessionInfo` that its row rendering should look
identical in style. `IdleInTransactionSession` rows are a simpler
two-field subtitle (`username`, `idle_duration_seconds`).

## A real layout bug this unit's own change exposed — fixed for real, not worked around

Adding a sixth zone caused a genuine `RenderFlex overflowed` error: the
prior layout was a fixed-height `Column` of six `Expanded` zones with
no scroll container, so once six zones' minimum content no longer fit
a typical test viewport (and would eventually not fit a real, modestly
sized application window either), content was silently clipped — a
real rendering bug, not a test-harness artifact, confirmed by the
exact yellow/black-striped overflow indicator Flutter renders for
this class of error. Two candidate fixes were considered:

- A top-level `ListView` (matching `DashboardScreen`'s own precedent
  for a scrollable multi-section screen) — rejected: `ListView` uses
  lazy sliver children, so zones below the fold aren't actually built
  into the element tree until scrolled into view, breaking every
  existing test's `await tester.pump(); expect(find.text(...))`
  pattern (confirmed by reproducing the exact failure: tests for the
  later zones — GUCs, deadlock history, long transactions,
  idle-in-transaction — found zero matching widgets after this change).
- **`SingleChildScrollView(child: Column(...))`** — the shape actually
  shipped. Unlike `ListView`, it always fully builds and lays out its
  single child eagerly, so every zone stays immediately findable in a
  test without a scroll gesture, while still allowing the whole screen
  to scroll once six zones' fixed heights exceed the viewport. Each
  zone's `Expanded` was replaced with a `SizedBox(height: 240)` so its
  own internal `ListView.builder` has the bounded height it needs
  (a `ListView.builder` inside an unbounded-height parent throws its
  own layout error).

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (2 new tests): a scripted
  long transaction renders its real pid/username/state/duration/query;
  a scripted idle-in-transaction session renders its real
  pid/username/idle-duration.
- All 8 existing tests — including every zone added by ADR
  0041/0043/0045 — still pass with the new `SingleChildScrollView`
  layout, confirmed by running the full suite after the layout change
  (not assumed from reading the diff).
- `flutter analyze`/`flutter test` clean (57 tests total, 2 new);
  console codegen-drift clean (`dart run tool/generate_models.dart` +
  `dart format lib/generated/models`); all 3 `.arb` locales updated
  together, every user-visible string routed through l10n.

## Consequences

- **This completes the console side of the entire DBMS wiring vein.**
  All nine multi-row/summary DBMS sections (sessions, locks, query
  stats, table stats, index stats, replication, GUCs, temp file
  activity, deadlock history) plus this unit's two (long transactions,
  idle-in-transaction sessions) are now visible end-to-end: core →
  service → console. No `DbmsAdapter` method lacks a console surface.
- `DatabaseScreen` is now a scrollable screen by design — any future
  DBMS section added here should keep using `SizedBox(height:
  zoneHeight, ...)` inside the existing `SingleChildScrollView`, not
  reintroduce a fixed-height `Expanded` split (which is exactly what
  broke this unit).
