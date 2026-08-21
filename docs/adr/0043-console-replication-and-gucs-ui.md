# 0043 — Console: Replication + GUCs UI

## Status

Accepted (unit U38).

## Context

U37 shipped `GET /api/v1/dbms/replication`/`/dbms/gucs` (ADR 0042) but
stayed service-only, explicitly naming the console side as future
work. This unit closes that gap, continuing the discipline established
by ADR 0039/0041.

## Decision

**Layout**: `DatabaseScreen` gains a fourth vertical zone — `Row
(Replication | Settings)` below the `Row (Table Stats | Index Stats)`
zone, same `_Section`/`VerticalDivider`/`Expanded` sizing as every
prior zone. Four zones total: `Row[Sessions|Locks]` → `QueryStats`
(full width) → `Row[TableStats|IndexStats]` → `Row[Replication|GUCs]`.

**A genuinely new row shape, not a plain list.** `ReplicationStatus{
is_primary, in_recovery, standbys: Vec<StandbyInfo>}` is the first
DBMS section whose data is a summary plus a nested list, unlike every
prior section (`SessionInfo`/`LockEdge`/`QueryStat`/`TableStat`/
`IndexStat`, all flat rows in a flat list). `_ReplicationSection`
renders a summary `ListTile` (`Primary`/`Standby (in recovery)`,
derived from `is_primary`) above a `Divider`, then the real `standbys`
list below via `_StandbysList`/`_StandbyRow` — reusing the same
empty-state (`l10n.genericEmpty`) and optional-field convention
(`clientAddr ?? '?'`, matching `_SessionRow`'s existing `username ??
'?'` precedent) every other list here already uses, rather than
inventing a new empty/null convention. `_GucsList`/`_GucRow` is a plain
list, the same shape as `_LocksList`/`_TableStatsList`.

**Single-object fetch, reusing an existing client pattern.**
`getDbmsReplication()` reuses `api_client.dart`'s existing
`getHostAnalysis()`/`getHealth()` single-object `_get(path,
T.fromJson)` shape — confirmed by direct read before writing it — not
the list-mapping shape `getDbmsSessions`/`getDbmsLocks` use.
`dbmsReplicationProvider` needed no provider-layer change either:
`analysis_providers.dart`'s `hostAnalysisProvider` already proves
`StreamProvider.autoDispose<ApiResult<T>>` works unchanged for a
single-object `T`.

**No derived judgment in the UI**, matching U36's own explicit
precedent: a standby's `replay_lag_seconds` is shown as-is (`?` when
null), not classified as "healthy"/"lagging" — that judgment, if ever
wanted, belongs server-side as a real, separate policy decision.

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (2 new tests): a scripted
  primary-with-one-standby replication response renders the real
  `Primary` summary plus the standby's real client address, state, and
  lag; a scripted GUC renders its real name/setting/source.
- All 6 existing tests (none of which ever scripted `/dbms/replication`
  or `/dbms/gucs`) still pass unchanged — same independent-section
  precedent ADR 0041 already established.
- `flutter analyze`/`flutter test` clean (53 tests total, 2 new);
  console codegen-drift clean (`dart run tool/generate_models.dart` +
  `dart format lib/generated/models`, matching ADR 0041's own
  documented post-generation formatting step); all 3 `.arb` locales
  updated together, every user-visible string routed through l10n.

## Consequences

- All seven DBMS slices with a shipped service endpoint (sessions,
  locks, query stats, table stats, index stats, replication, GUCs) are
  now fully wired end-to-end: core → service → console.
- The real standby-populated replication path (ADR 0042's own named
  gap: no streaming-replication fixture in this project's test
  harness) still can't be proven against a live server, but the
  *rendering* of a standby row is now proven with real, scripted data
  — the console-layer gap this ADR could close is closed; the
  core/service-layer gap ADR 0042 named remains open, unrelated to this
  unit's scope.
- `temp_file_activity`, `deadlock_history`, `long_transactions`, and
  `idle_in_transaction_sessions` all remain real, separate future
  `DbmsAdapter` slices, each needing its own service endpoint before
  its own console section.
