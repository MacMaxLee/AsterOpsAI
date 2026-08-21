# 0037 — Console: Database Screen (Sessions + Locks)

## Status

Accepted (unit U32).

## Context

U31 shipped `GET /api/v1/dbms/sessions`/`GET /api/v1/dbms/locks`, the
first direct DBMS data surface, but stayed service-only. This unit adds
the console side: a new "Database" nav destination, the first place in
the running product an operator can see raw session/lock state
directly rather than only correlation's own filtered verdict.

## Decision

**One screen, two independent sections, not tabs.** Mirrors
`DashboardScreen`'s own established shape — several independently-
polled `AsyncResultView`s stacked together — rather than inventing a
new tabbed-navigation pattern. A slow or failed poll on one section
(e.g. `locks`, if the DB briefly can't answer that query) never blocks
the other (`sessions`) from rendering, since each watches its own
provider (`dbmsSessionsProvider`/`dbmsLocksProvider`,
`console/lib/providers/dbms_providers.dart`, a new file matching how
`policy_providers.dart`/`tuning_providers.dart`/`security_providers.dart`
are each already their own file rather than folded into
`telemetry_providers.dart`).

**Read-only, no mutation UI.** Both `/dbms/sessions` and `/dbms/locks`
are `GET`-only endpoints (U31) — there is nothing to propose, grant, or
suspend here. This screen follows `TuningHistoryScreen`/
`SecurityIncidentsScreen`'s own read-only list shape, not
`ProcessesScreen`'s confirm-dialog/mutation shape.

**Every field rendered verbatim** (FR-CONSOLE-001): `session.state`/
`lock.lockType` are shown as the real server string (`SessionState`'s
generated `wireValue`, e.g. `"ACTIVE"`, `"WAITING"`), never recolored
or reclassified — no new badge/icon component was introduced for this
screen, matching how `TuningHistoryScreen` already shows raw `status`
text with no added presentation.

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (NEW, 3 tests): a scripted
  session renders its real pid/username/database/state/query verbatim;
  a scripted lock renders its real blocked/blocking pids and lock type;
  an empty response for both sections shows the real empty state
  independently in each section (`findsNWidgets(2)` — proving the two
  sections are genuinely separate, not one shared empty-state check).
- `flutter analyze`/`flutter test` clean (47 tests, 3 new); `dart
  format` clean; console codegen-drift clean (new `SessionInfo`/
  `SessionState`/`LockEdge` models regenerated from U31's own committed
  schemas).

## Consequences

- An operator can now see raw DB session and lock state directly in
  the console, not only correlation's filtered summary — the first
  console surface for the DBMS area of the product at all.
- The remaining `DbmsAdapter` methods U31 deferred (`query_stats`,
  table/index stats, replication, GUCs, deadlocks, long transactions,
  idle-in-transaction sessions) all remain real, separate future
  slices, each needing its own console section once its own service
  endpoint exists.
