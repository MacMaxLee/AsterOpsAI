# 0041 — Console: Table + Index Stats UI

## Status

Accepted (unit U36).

## Context

U35 shipped `GET /api/v1/dbms/table-stats`/`/dbms/index-stats` (ADR
0040) but stayed service-only, explicitly naming the console side as
future work. This unit closes that gap, continuing the discipline
(established by ADR 0039/U34) of not letting unconsumed backend
surface run too far ahead of the console.

## Decision

**Layout**: `DatabaseScreen` gains a third vertical zone — a new `Row
(Table Stats | Index Stats)` below the existing `Query Stats` section,
mirroring the existing `Row (Sessions | Locks)` shape exactly (same
`_Section`/`VerticalDivider`/`Expanded` sizing). No new layout pattern
invented: three zones total, `Row[Sessions|Locks]` →
`QueryStats` (full width) → `Row[TableStats|IndexStats]`.

**No `GatedValue<T>` handling needed here**, unlike U34's query stats
section — `table_stats`/`index_stats` return plain `Vec<T>` at every
layer (confirmed by U35/ADR 0040), so `_TableStatsList`/`_IndexStatsList`
reuse the exact plain-list pattern `_SessionsList`/`_LocksList` already
established, not the four-state gated-message pattern.

**Row content is the real fields, not a derived judgment**: a table
row shows `seq_scan`/`idx_scan`/`n_live_tup`/`n_dead_tup` (the raw
bloat/usage signal); an index row shows its owning table and
`idx_scan`. An index with `idx_scan == 0` is the classic "candidate for
dropping" signal a DBA looks for, but this unit deliberately does not
compute or label that judgment in the UI — surfacing the real number
lets the human decide, and any future "flag likely-unused indexes"
feature is a real, separate policy decision belonging server-side (the
same principle already applied to gated-capability rendering: show
what's real, don't invent a verdict the backend hasn't made).

**Models generated, not hand-written**: `dart run
tool/generate_models.dart` (from the already-committed U35 schemas)
produced `TableStat`/`IndexStat`, followed by `dart format
lib/generated/models` — running the generator alone reformats every
existing generated file to its raw (unformatted) output; the
project's own `scripts/check-console-codegen-drift.sh` already runs
`dart format` immediately after generation for exactly this reason,
confirmed by reading the script before assuming drift.

## Verification (real, not simulated)

- `console/test/database_screen_test.dart` (2 new tests): a scripted
  table stat renders its real schema/table/seq_scan/idx_scan/live/dead
  fields; a scripted index stat renders its real index/table/idx_scan
  fields.
- All 5 existing tests (none of which ever scripted `/dbms/table-stats`
  or `/dbms/index-stats`) still pass unchanged — `AsyncResultView`
  renders an unscripted poll as a real per-section failure message
  (`FakeTransport` returns `ApiFailureUnavailable`, not a thrown
  error), confirming the new sections are independent of the existing
  ones exactly like every prior DBMS section pairing.
- `flutter analyze`/`flutter test` clean (51 tests total, 2 new);
  `dart format` clean; console codegen-drift clean; all 3 `.arb`
  locales updated together, every user-visible string routed through
  l10n (no hardcoded English left in `_TableStatRow`/`_IndexStatRow`).

## Consequences

- All five DBMS slices shipped so far (sessions, locks, query stats,
  table stats, index stats) are now fully wired end-to-end: core →
  service → console.
- Replication status, relevant GUCs, temp-file activity, deadlock
  history, long transactions, and idle-in-transaction sessions remain
  real, separate future `DbmsAdapter` slices, each needing its own
  service endpoint before its own console section.
