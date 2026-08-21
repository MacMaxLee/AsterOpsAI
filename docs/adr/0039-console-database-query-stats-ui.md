# 0039 — Console: Query Stats Section

## Status

Accepted (unit U34).

## Context

U33 shipped `GET /api/v1/dbms/query-stats`, but stayed service-only.
This unit adds the console side, closing the loop before any more
backend `DbmsAdapter` slices — the established discipline of not
letting unconsumed backend surface run too far ahead of the console.

## Decision

**Layout restructure**: `DatabaseScreen` moves from a single `Row
(Sessions | Locks)` to a `Column` with two halves — the existing `Row`
on top, a new, full-width `_QueryStatsSection` below. Query text needs
real horizontal room a third narrow column wouldn't give it; each half
gets equal flex via `Expanded`, matching the sizing already used within
each individual section.

**A genuinely new UI shape, reusing an existing vocabulary rather than
inventing one.** Unlike `sessions`/`locks` (plain lists), this
endpoint's data is `GatedValueForArrayOfQueryStat` (the generated Dart
shape for `contracts::GatedValue<Vec<QueryStat>>`, unit U33) — four
states, not one list. `console/lib/widgets/metric_display.dart`'s
`MetricValueText` already established the exact same four-state
vocabulary (via `Capability`) with existing, reusable l10n strings
(`metricStateUnavailableTitle`/`metricStateLimitedTitle`/
`metricStatePermissionRequiredTitle`) and icon choices
(`Icons.remove_circle_outline`/`Icons.info_outline`/`Icons.lock_
outline`). `_QueryStatsSection`'s non-`Supported` states reuse those
exact strings and icons rather than inventing new copy — even though
`MetricValueText` itself can't be reused directly (it renders one
inline value; this needs a whole list-or-message section), the visual
vocabulary stays identical everywhere a gated state appears in the
console.

**Every row field goes through l10n**, matching this project's own
consistent discipline (even `_SessionRow`/`_LockRow`'s simple "PID:"/
"State:" labels already go through l10n) — a first draft of
`_QueryStatRow` used hardcoded English `'calls: ...'` text directly;
caught and fixed before committing by re-reading the existing
`dbmsColumnPid`/`dbmsSessionRowSubtitle` precedent this same file
already set in U32, not by an external review.

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (2 new tests): a scripted
  `Supported` response renders the real query text, call count, and
  mean time; a scripted `Unavailable` response (mirroring U33's own
  real `"pg_stat_statements"`-naming reason) shows that real reason
  text verbatim, not a fabricated generic message.
- All 3 existing tests (which never scripted `/dbms/query-stats` at
  all) still pass unchanged — confirms the three sections are
  genuinely independent: an unscripted/failing query-stats poll never
  blocks sessions/locks from rendering, or vice versa.
- `flutter analyze`/`flutter test` clean (49 tests, 2 new); `dart
  format` clean; console codegen-drift clean.

## Consequences

- All three DBMS slices shipped so far (sessions, locks, query stats)
  are now fully wired end-to-end: core → service → console.
- `GatedValue<T>`'s console rendering pattern (reuse `metric_display.
  dart`'s existing vocabulary) is now established for any future
  endpoint wrapping a gated capability, not just this one.
- Table/index stats, replication status, relevant GUCs, temp-file
  activity, deadlock history, long transactions, and idle-in-
  transaction sessions all remain real, separate future `DbmsAdapter`
  slices, each needing its own service endpoint before its own console
  section.
