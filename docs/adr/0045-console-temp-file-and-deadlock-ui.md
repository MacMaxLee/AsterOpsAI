# 0045 — Console: Temp File Activity + Deadlock History UI

## Status

Accepted (unit U40).

## Context

U39 shipped `GET /api/v1/dbms/temp-file-activity`/`/dbms/deadlock-history`
(ADR 0044) but stayed service-only, explicitly naming the console side
as future work. This unit closes that gap, continuing the discipline
established by ADR 0039/0041/0043.

## Decision

**Layout**: `DatabaseScreen` gains a fifth vertical zone — `Row(Temp
File Activity | Deadlock History)` below the `Row(Replication |
Settings)` zone, same `_Section`/`VerticalDivider`/`Expanded` sizing as
every prior zone.

**Flat summaries, not lists.** `TempFileActivity{temp_files,
temp_bytes, stats_reset}` and `DeadlockInfo{deadlocks, stats_reset}`
each describe exactly one object per poll, not a collection — unlike
every prior section (including `ReplicationStatus`'s nested
`standbys`), `_TempFileActivitySection`/`_DeadlockHistorySection` are a
small centered `Column` of labeled `Text` lines, not a `ListView`.

**Reused existing formatting utilities rather than inventing new
ones**: `console/lib/widgets/formatters.dart`'s existing `formatBytes(int)`
renders `temp_bytes` (the same helper `dashboard_screen.dart`/
`memory_screen.dart`/`storage_screen.dart` already use for every other
byte value in the console); `stats_reset: Option<DateTime>` uses
`tuning_history_screen.dart`'s own `DateFormat.yMd().add_Hm()` pattern.

**Null `stats_reset` renders nothing, not a placeholder** — the same
"never fabricate" precedent `tuning_history_screen.dart` already
established for its own nullable `completedAt` (a plan's own test is
named exactly for this: *"a plan with no completedAt never shows a
fabricated completion time"*). A null `stats_reset` is real information
(PostgreSQL hasn't reset these particular counters since the cluster
started), not missing data to paper over.

**Single-object fetch, reusing U38's own pattern**: `getDbmsTempFileActivity()`/
`getDbmsDeadlockHistory()` both reuse `getDbmsReplication()`'s
`getHostAnalysis`-style `_get(path, T.fromJson)` shape; no provider-layer
change needed beyond two more `StreamProvider.autoDispose<ApiResult<T>>`
entries, identical to `dbmsReplicationProvider`.

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (2 new tests): a scripted
  temp-file-activity response with a real non-null `stats_reset`
  renders the real file count, real formatted bytes (`formatBytes`
  applied to a real value), and the real reset-time line; a scripted
  deadlock-history response with a real null `stats_reset` renders the
  real deadlock count and asserts the reset-time line is genuinely
  absent (`findsNothing`), not fabricated — mirroring the exact
  assertion style `tuning_history_screen_test.dart`'s own null-`completedAt`
  test already uses (checking for the label's absence, not asserting
  on a timezone-dependent formatted date string).
- All 8 existing tests (none of which ever scripted
  `/dbms/temp-file-activity` or `/dbms/deadlock-history`) still pass
  unchanged — same independent-section precedent ADR 0041/0043 already
  established.
- `flutter analyze`/`flutter test` clean (55 tests total, 2 new);
  console codegen-drift clean (`dart run tool/generate_models.dart` +
  `dart format lib/generated/models`); all 3 `.arb` locales updated
  together, every user-visible string routed through l10n.

## Consequences

- All nine `DbmsAdapter` slices with a shipped service endpoint
  (sessions, locks, query stats, table stats, index stats, replication,
  GUCs, temp file activity, deadlock history) are now fully wired
  end-to-end: core → service → console.
- Only `long_transactions`/`idle_in_transaction_sessions` remain as a
  real, separate future `DbmsAdapter` slice needing both a service
  endpoint and its own console section — the last gap in this vein.
