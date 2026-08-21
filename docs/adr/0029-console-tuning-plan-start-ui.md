# 0029 — Console: Tuning Plan Start UI

## Status

Accepted (unit U24).

## Context

U23 wired `POST /api/v1/tuning/start` on the service side but was
deliberately service-only — the console's `TuningHistoryScreen` (U17)
stayed read-only, with no way for an operator to actually start a plan
from the running product. This unit adds that: a real UI, on a real live
process, calling the real U23 endpoint, showing the real (and, per ADR
0028, sometimes surprising) outcome verbatim.

## Environment correction

Earlier in this session, `flutter test` failed with `command not found`
and was reported as an environment limitation. Direct investigation this
unit found that was a `$PATH` issue, not a missing SDK: Flutter 3.47.0 /
Dart 3.13.0 are installed at `/home/parallels/flutter/bin`, added to
`$PATH` by `~/.bashrc` for interactive shells but not present in this
tool's non-interactive shell environment. `flutter analyze`/`flutter
test`/`dart format` all ran clean once that path was added explicitly.
Every prior console unit in this session was presumably built and
verified under an interactive shell where this wasn't an issue; this is
the first time it was hit and diagnosed directly rather than assumed.

## Decision

**Entry point: a per-row action on `ProcessesScreen`, not a new nav tab
or a target picker.** `ProcessInfo` already carries both `pid` and
`startTimeTicks` — exactly `TargetIdentity::Process`'s two fields — live
in the UI today via the existing `processesProvider`. Starting a plan
from the process row itself means the operator never has to type a PID
or a start-time tick count by hand, which they couldn't know anyway (it
comes from `/proc/[pid]/stat` field 22). `_ProcessRow` gained a trailing
`Icons.tune_outlined` icon button opening `_StartTuningPlanDialog`,
following the exact `showDialog` → `AlertDialog` + `Form` shape
`SecurityIncidentsScreen`'s `_SuppressDialog` already established.

**Dialog fields**: `resource_name` (required text, defaulting to the
row's `comm`), `profile` and `mode` as `DropdownButtonFormField<String>`
over the exact wire string values `service/src/api/v1/tuning.rs`'s
`WireTuningProfile`/`WireAutomationMode` already use — a plain `String`
selection, not a client-side enum, so a new profile/mode added to the
server's wire contract only needs this dropdown's literal list updated,
never a parser. `pid`/`start_time_ticks` are fixed from the row, never
editable. `requested_by` is the same `'console-operator'` placeholder
every other console mutation already uses (ADR 0018/0021/0027's
repeatedly-flagged no-auth-system gap).

**A result dialog, not a snackbar.** Grant/reject/suppress all return
`()` and a snackbar is enough. Starting a plan returns real, meaningful
data, and ADR 0028's own central finding is that the real outcome is
often surprising (`AUTO_ALLOWED_PENDING`, not `PENDING_APPROVAL`, for
every action type registered today). A snackbar saying "plan started"
would silently launder that surprise into an apparent success; a second
`AlertDialog` (`_TuningPlanResultDialog`) lists every real
`TuningCandidateOutcome`'s `action_type`/`outcome` verbatim
(`{actionType} — {outcome}`, via the new `tuningStartResultCandidate`
l10n string) — never recolored, iconified, or reclassified
(FR-CONSOLE-001), the same discipline `TuningHistoryScreen`/
`SecurityIncidentsScreen` already apply to `status`/`severity`. A real
failure shows inline in the form dialog instead (same as
`_SuppressDialog`), so the operator can see the error and resubmit
without losing their inputs.

**A new `startTuningPlan` on `ApiClient` needed a new `_post<T>` helper**
— the existing `_postForSuccess` only ever decodes `()`. `_post<T>`
mirrors `_get<T>`, reusing the same `_decodeEnvelope<T>` — real data,
real envelope, real error mapping, not a second decode path.

**Codegen**: `contracts::TuningPlanOutcome`/`TuningCandidateOutcome`
(U23) were already schema-emitted and committed; this unit just ran
`dart run tool/generate_models.dart` to pick them up — no hand-written
DTOs.

## Real, pre-existing gap discovered (not fixed, out of scope)

Running `dart format --output=none --set-exit-if-changed .` (the exact
CI command) flagged `lib/screens/correlation_screen.dart` and
`test/correlation_screen_test.dart` as needing reformatting — confirmed
via `git status` to be **completely unrelated to this unit's own
changes**: both files are untouched by this unit and identical to what's
already committed on `main`. This means the console's `format` CI gate
is presumably already failing (or was formatted with a slightly
different Dart SDK version than what's installed here) independent of
this unit. Left alone — fixing unrelated files is out of this unit's
scope (its own FORBIDDEN list: no changes to other screens) — but
flagged here rather than silently worked around, for a future unit or a
direct `dart format` pass to pick up.

## Verification (real, not simulated)

- `ProcessesScreen` previously had zero test coverage — a real,
  pre-existing gap this unit also closes. `test/processes_screen_test.dart`
  (NEW) covers the existing read-only list rendering plus:
  - the dialog pre-fills `resource_name` with the row's real `comm`;
  - submitting posts the real JSON body (`pid`, `start_time_ticks`,
    `resource_name`, `profile`, `mode`, `requested_by`) to
    `/api/v1/tuning/start`, asserted against `transport.postedRequests`;
  - a scripted success response renders each real candidate's
    `action_type`/`outcome` verbatim in the result dialog;
  - a scripted real `400` shows the inline error and keeps the dialog
    open — no crash, no silent dismiss.
- `flutter analyze`: clean. `flutter test`: all 38 tests pass (33
  pre-existing + 5 new). `dart format --output=none --set-exit-if-changed`:
  clean for every file this unit touched.
- `scripts/check-console-codegen-drift.sh`: clean once this unit's
  generated-model changes are committed (confirmed the only diff before
  committing was the two new files' own barrel-export lines).

## Consequences

- An operator can now start a real tuning plan against a real live
  process from the running console, and see the real, sometimes
  surprising outcome honestly — closing the gap ADR 0028 named.
- `ProcessesScreen` gains real test coverage for the first time.
- The pre-existing `correlation_screen.dart`/`correlation_screen_test.dart`
  format-gate gap remains open, named above, not silently fixed here.
