# 0055 — Policy Inbox: Reach `AUTO_ALLOWED_PENDING` Rows Under Development

## Status

Accepted (unit U50).

## Context

ADR 0028 (tuning-plan-start-wiring, U24) named a real gap: a proposed
action that policy auto-allows (`PolicyOutcome::AutoAllowed`, SQLite
status `AUTO_ALLOWED`) is real, audited, and has a real row in the
`actions` table — but was genuinely unreachable from every HTTP
surface. ADR 0030 (policy-environment-configuration, U25) closed this
only for a *configured* `Environment::Production`; it explicitly left
the default, unconfigured `Environment::Development` posture — today's
actual default for every deployment that hasn't set
`$ASTEROPS_POLICY_ENVIRONMENT` — still exhibiting the original gap.
Confirmed by direct read before implementing: no later ADR had closed
it.

**Confirmed to be a missing-action gap, not a missing-visibility one**,
by tracing the real code before designing a fix:

- `core::policy::risk::decide`: `RiskLevel::Low` + `Environment::
  Development` → `PolicyDecision::AutoAllow`. Two real, already-
  registered action types hit this today (`host.set_process_cpu_
  affinity`, `host.set_process_priority`, both `RiskLevel::Low`).
- `core::policy::evaluate::evaluate` writes the row with status
  `AUTO_ALLOWED` and does nothing else — nothing in `core` auto-executes
  it (the only auto-execution path in this codebase,
  `core::tuning::plan.rs`'s own narrower `should_auto_execute` gate, is
  unrelated to a direct `/api/v1/actions/propose` call). So a Low-risk
  action proposed directly, under the real default Development
  environment, produced a row that sat inert forever.
- `core::repository::policy::list_pending_approval`: `WHERE status =
  'PENDING_APPROVAL'` only.
- `core::policy::approval::grant`'s CAS required the row's current
  status be `PENDING_APPROVAL`.
- `core::policy::approval::authorize`, by contrast, already accepted a
  row whose status was `AutoAllowed` *or* `Approved` directly — its own
  doc comment already said "(or AUTO_ALLOWED, which needs no separate
  grant)". The execution mechanism was already there; only the
  application-layer gates above blocked it.

This is general `core::policy` lifecycle scope, not tuning-specific,
despite ADR 0028's tuning-focused title: `core::tuning::plan.rs` and
the standalone `/api/v1/actions/propose` endpoint both call the same
`evaluate`/`approval` module and write to the same `actions` table.

## Decision

**Teach the inbox/grant path about `AUTO_ALLOWED`** — ADR 0028's own
third named option, exactly the one ADR 0030 left open:

- `core::repository::policy::list_pending_approval`: widened to
  `WHERE status IN ('PENDING_APPROVAL', 'AUTO_ALLOWED')`.
- `contracts::policy::PendingActionSummary`: new `status: String`
  field, matching `ActionProposalOutcome.status`'s own established
  precedent in the same file — a plain string, not a new enum type.
- `service`'s `grant` handler: fetches the row first, branches on its
  *current* status. `AUTO_ALLOWED` → skips `approval::grant` entirely
  (the row is already policy-authorized; `grant`'s CAS would always
  reject it) and records a new, distinct audit event
  (`policy.auto_allowed_executed`) for the same FR-POL-005 parity
  `grant`'s own `policy.granted` event provides — since skipping
  `grant` also skips that audit call. Any other status (including
  `PENDING_APPROVAL`) falls through to the unchanged `approval::grant`
  call, with unchanged errors. Both branches then proceed through the
  existing, unmodified `authorize` + `execute` calls.
- Console (`policy_inbox_screen.dart`): reads the new `status` field.
  An `AUTO_ALLOWED` row hides the Reject button (its CAS is untouched —
  rejecting one would always 400) and relabels the action button "Run
  now" (new l10n key `policyRunNow`) — same underlying `grantAction`
  call; the server-side branch above handles the rest.

## A real, pre-existing test that documented the gap — updated, not deleted

`service/tests/tuning_endpoints.rs::ask_before_changes_creates_a_real_
row_the_existing_inbox_cannot_yet_reach` (from ADR 0028's own unit)
explicitly asserted the *old, broken* behavior: an `AUTO_ALLOWED_
PENDING` candidate must not appear in `/api/v1/policy/pending`, and
granting it must 400. This unit's fix inverted both assertions, so the
test was rewritten — renamed to `ask_before_changes_creates_an_auto_
allowed_row_now_reachable_from_the_inbox` — to prove the new, correct
behavior instead: the row is now visible with `status: "AUTO_ALLOWED"`,
and granting it now genuinely changes the real target's CPU affinity —
the same loop `environment_production_makes_ask_before_changes_really_
reach_the_inbox_and_grantable` (ADR 0030/U25) already proved for
`Production`, now proven for the real default `Development` too.

## Verification (real, not simulated)

- `service/tests/policy_endpoints.rs`: new `insert_auto_allowed_for`
  helper (mirrors the existing `insert_pending_for`). New tests: a real
  `AUTO_ALLOWED` row appears in `/api/v1/policy/pending` with the
  correct `status`; granting one genuinely changes the real target's
  CPU affinity mask (not just a row-status assertion) and records the
  new `policy.auto_allowed_executed` audit event — verified via a
  direct `audit_events` table read (`repository::reader::checkout`),
  since `actions::execute`'s own trailing `policy.executed` event
  becomes the *latest* event, so `latest_audit_event_type` alone can't
  prove the new event exists; a pre-existing test
  (`granting_an_already_granted_action_is_a_bad_request_not_a_panic`)
  already proves the non-`AUTO_ALLOWED` path's `WrongStatus` 400 is
  untouched, since it exercises the new pre-check branch against an
  already-`EXECUTED` row.
- `service/tests/action_propose_endpoints.rs`: one new real,
  *unforced* end-to-end capstone — `host.set_process_cpu_affinity`
  (genuinely `RiskLevel::Low`) proposed under this file's own real
  default `Environment::Development` genuinely produces
  `ActionProposalOutcome.status == "AUTO_ALLOWED"` (no test-only status
  manipulation), now appears in the inbox, and granting it really
  changes the real spawned target's CPU affinity — proof the whole gap
  is closed end-to-end, over real HTTP, under the real default
  environment.
- `console/test/policy_inbox_screen_test.dart`: new test — an
  `AUTO_ALLOWED` row renders "Run now" with no "Reject", and tapping it
  posts to the same `/grant` endpoint.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `flutter analyze`/`flutter test` clean (69 console tests).
  Both grep gates clean. Schema-drift clean (schema regenerated and
  committed alongside the `contracts` change). `cargo deny check`/
  `cargo audit` clean. No leaked PostgreSQL processes.

## Consequences

- Closes the gap ADR 0028 named and ADR 0030 left open for the default
  `Development` environment — every real, already-registered Low-risk
  action type now has a genuine, human-triggered path from
  `AUTO_ALLOWED_PENDING` to actually executed.
- **Deliberately deferred, not silently dropped**: rejecting/vetoing an
  `AUTO_ALLOWED` row before it's run is a real, distinct capability
  this unit does not add — `approval::reject`'s own CAS still only
  accepts a `PENDING_APPROVAL` row, unchanged. What "rejecting an
  already-policy-authorized action" should even mean/audit is a real
  design question, not a mechanical extension of this unit's fix; a
  future unit's own scope.
- ADR 0028's other two named options (a Medium/High-risk action type;
  further `Environment` configuration surfaces) remain untouched —
  this closes option three specifically.
