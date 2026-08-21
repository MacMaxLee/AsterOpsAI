# 0034 — Core+Service+Console: Resumable-Action Discovery + Resume UI

## Status

Accepted (unit U29).

## Context

U28 wired `POST /api/v1/policy/{id}/rollback`, but calling it needs a
row id, and nothing could find one: the Policy inbox only ever lists
`PENDING_APPROVAL` rows, and a granted suspend moves to `EXECUTED` and
disappears from it entirely. Presented three real design options (a
per-process Resume button with a new, scoped backend lookup; a new
"Recent Actions" browse screen; showing it only right after granting)
— the user picked the first: smallest real scope, and the same
discoverable per-row pattern already established twice (U24's tune,
U27's suspend).

## A real correctness subtlety

`core::actions::rollback::rollback` (U11/U28) never mutates the
original row — a successful rollback inserts a *new* row with
`rollback_of: Some(original_id)`, transitioned to `ROLLED_BACK`. A
naive "most recent `EXECUTED` row for this target" query would keep
offering "Resume" forever, even after the process was already resumed.
The real discovery query (`core::repository::policy::find_resumable_
action`) excludes any row that already has a `ROLLED_BACK` child via a
`NOT IN (SELECT rollback_of FROM actions WHERE rollback_of IS NOT NULL
AND status = 'ROLLED_BACK')` subquery — a `FAILED` rollback attempt
does *not* exclude the original, since a failed attempt means the
target is presumably still in its executed (e.g. suspended) state and
must stay resumable. Verified for real, not just reasoned about: a new
service test grants a real suspend, confirms the row appears as
resumable, rolls it back for real, and confirms it stops appearing.

## A real, named edge case, not fixed here

Nothing prevents the same target being suspended twice (proposed and
granted twice) before either is rolled back — no concurrency guard
exists for `/actions/propose` the way `core::tuning`'s partial `UNIQUE`
index guards tuning plans. `find_resumable_action` picks the most
recent un-rolled-back `EXECUTED` row (`ORDER BY created_at DESC LIMIT
1`) — the same "current state wins" semantics used elsewhere in this
project, not a new guard. Named here as real, minor, separate future
work, not silently ignored.

## Decision

**New service endpoint `GET /api/v1/actions/resumable?pid=X&
start_time_ticks=Y`** (`axum::extract::Query`, mirroring `history.rs`'s
own query-param pattern), returning `ApiResponse<Vec<
ResumableActionSummary>>` — a **list**, not `Option<T>`: the existing
`Envelope`/console decoder treats "success with null data" as
malformed, confirmed by reading `_decodeEnvelope<T>` before choosing
this shape, so "nothing to resume" is an empty list, the same
convention `PendingActionSummary`'s own listing already uses, not a
new null-data special case.

**Console: on-demand lookup, not background polling.** `ProcessesScreen`
already polls `/processes` continuously; polling `/actions/resumable`
in the background for every visible row would be a real, avoidable
cost. Instead, a third per-row icon button opens `_ResumeProcessDialog`,
which fires the lookup exactly once, on the dialog's own first frame.
Found: the real `action_type`/`executed_at` shown verbatim
(FR-CONSOLE-001), with a Resume button that calls the *already-existing*
`POST /policy/{id}/rollback` (U28) using the discovered row id. Not
found: an honest "nothing to resume" message, Close only. Success: pop
+ a snackbar naming the real `action_type`/row id — matching
`PolicyInboxScreen`'s own grant/reject precedent (void-returning
mutations get a snackbar, not a result dialog; U27's suspend got a
result dialog specifically because *propose* returns real status data,
which rollback, like grant/reject, does not).

## A real bug found by testing, not by reading code

The first implementation fired the lookup from `initState()`, calling
`AppLocalizations.of(context)!` there. Flutter raised a real assertion
immediately in the very first test: `Localizations.of` depends on an
`InheritedWidget`, unavailable until after `initState` completes —
calling it there is simply illegal, not a style nit. Fixed by moving
the one-shot lookup to `didChangeDependencies` (guarded by a
`_checkStarted` flag, since that method can fire more than once) — the
Flutter-idiomatic place for dependency-requiring initialization.
Caught by the very first widget test run against real dialog code, not
by review.

## Verification (real, not simulated)

- `core`-level: none added directly (the query is exercised through
  the service test below, which is the real end-to-end path this unit
  cares about — the SQL itself has no independent unit worth adding
  beyond that live proof).
- `service/tests/action_propose_endpoints.rs`
  (`a_resumable_action_appears_after_grant_and_disappears_after_
  rollback`): empty before anything executes; a real propose+grant
  makes the row appear with its real `row_id`/`action_type`/
  `executed_at`; a real rollback (U28's own endpoint) makes it
  disappear again — proves the `NOT IN (... ROLLED_BACK)` exclusion
  for real.
- `console/test/processes_screen_test.dart` (3 new tests): the lookup
  fires with the row's real `pid`/`start_time_ticks` on dialog open; an
  empty result shows the real not-found message with no Resume button;
  a found result shows the real `action_type` and, on confirm, posts
  the real rollback body and shows the real success snackbar; a real
  error keeps the dialog open with an inline message, not a crash.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift clean (new type + list
  envelope committed); `flutter analyze`/`flutter test` clean (44
  console tests); `dart format` clean; console codegen-drift clean;
  `cargo deny check`/`cargo audit` clean; no leaked processes.

## Consequences

- The full propose → grant → execute → discover → rollback loop is now
  real end-to-end through the console for the first time: an operator
  can suspend a process and later find and resume it without knowing
  or remembering a row id.
- The double-suspend edge case (named above) and `host.*` rollback's
  own `previous_state`-loss limitation (ADR 0033) both remain open,
  real, separate future work.
- No "browse all actions" screen exists — deliberately, per the
  user's own choice of the smaller-scoped option.
