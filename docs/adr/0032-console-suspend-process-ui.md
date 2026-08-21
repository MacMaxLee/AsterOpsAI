# 0032 — Console: Suspend Process UI

## Status

Accepted (unit U27).

## Context

U26 wired `POST /api/v1/actions/propose` and registered `security.
suspend_process` (real `SIGSTOP`, `RiskLevel::High`) at the service
level, but stayed service-only. This unit adds the console-side
affordance: a real "suspend this process" action on `ProcessesScreen`.

## Decision

**A second per-row icon button, not a form.** `_StartTuningPlanDialog`
(U24) collects a resource name, profile, and mode because `/tuning/
start` genuinely needs them. `security.suspend_process` takes no
parameters at all (`core::security::response::validate_no_parameters`,
unit U26) — there is nothing to configure. The right shape for a
disruptive, parameterless action is a plain confirmation, not an empty
`Form`: `_SuspendProcessDialog` keeps the same `ConsumerStatefulWidget`
+ loading-state + inline-error shape as `_StartTuningPlanDialogState`,
but its body is descriptive text, not fields. `pid`/`start_time_ticks`
come from the same real `ProcessInfo` row U24's dialog already reads —
never typed by hand.

**Result shown honestly, not a snackbar** — the same reasoning ADR 0029
already gave for the tuning result dialog, reused verbatim here:
`security.suspend_process` is always `PENDING_APPROVAL` (`RiskLevel::
High` never auto-allows), and that's meaningful, not a plain "done."
`_ActionProposalResultDialog` renders `status`/`row_id` verbatim
(FR-CONSOLE-001) — no added narrative about what to do next; the status
string already says `PENDING_APPROVAL`, and the existing Policy inbox
screen (U16) is where an operator would look regardless.

**No new grant/execute wiring anywhere.** The row this dialog creates
lands in the same `actions` table U16's Policy inbox screen already
polls and already knows how to grant — this unit adds exactly one new
capability (propose) and reuses everything downstream of it unchanged.

**`ApiClient.proposeAction`** reuses the `_post<T>` generic helper U24
already added for `startTuningPlan` — no new client plumbing.
`parameters` is never sent in the request body at all (the server
already defaults a missing field to `{}`, unit U26) rather than sending
an explicit empty object — one less thing for a future non-tuning
action type with real parameters to accidentally inherit as "always
omit."

## Verification (real, not simulated)

- `the suspend confirmation dialog shows the row's real comm and pid`:
  confirms the dialog reads the real, already-fetched `ProcessInfo` row,
  not a placeholder.
- `confirming suspend posts the real security.suspend_process proposal
  and renders the real status/row_id verbatim`: asserts the real JSON
  body (`action_type`, `pid`, `start_time_ticks`, `resource_name`,
  `requested_by`, and the deliberate *absence* of a `parameters` key)
  against `transport.postedRequests`, and that the result dialog shows
  the scripted `PENDING_APPROVAL`/row id exactly.
- `a real error suspending shows an inline message and keeps the dialog
  open`: a scripted `400` shows inline, the confirmation dialog stays
  open and resubmittable — no crash, no silent dismiss.
- `flutter analyze`/`flutter test` clean (41 tests, all pre-existing
  plus 3 new); `dart format --output=none --set-exit-if-changed` clean
  for every file this unit touched (the pre-existing, unrelated
  `correlation_screen.dart` drift ADR 0029 already flagged remains
  untouched, out of this unit's scope); console codegen-drift gate
  clean once committed.

## Consequences

- `security.suspend_process` — wired at the service level in U26 but
  unreachable from the running product — is now a real, live console
  capability: propose from Processes, approve from Policy, both already
  built, now connected end-to-end for the first time.
- Two of ADR 0028's three named gaps are now closed with working
  product surfaces on both ends (service in U25/U26, console in U24/
  U27). The third — teaching the inbox/grant path to recognize
  `AUTO_ALLOWED_PENDING` rows for Low-risk tuning candidates under the
  default environment — remains open, real, separate future work.
