# 0059 — Policy Inbox: Reject/Veto an `AUTO_ALLOWED` Row

## Status

Accepted (unit U54).

## Context

U50 (ADR 0055) made `AUTO_ALLOWED` rows reachable from the policy
inbox and runnable via "Run now," but explicitly deferred the
complementary capability: a human had no way to veto one before it
ran. `approval::reject`'s CAS still only accepted a `PENDING_APPROVAL`
starting status, so an unwanted `AUTO_ALLOWED` row just sat inert in
the inbox forever. ADR 0055's own FORBIDDEN section named this
precisely as a real, deferred design question, not a mechanical
extension. Confirmed still open by this session's fresh, full ADR
re-audit — no later ADR touched it.

## Decision

**The design question — what "rejecting an already-policy-authorized
action" means — resolved by direct comparison with `authorize`'s own
existing precedent.** `approval::authorize` already accepted a row
whose status was `Approved` *or* `AutoAllowed` directly — the identical
"two valid starting statuses, one CAS target" shape `reject` needed.
Reusing `DENIED` as the terminal status (rather than inventing a new
status value) was the minimal, most consistent choice: `DENIED`
already means "policy will not execute this," equally true whether a
human declined a pending proposal or vetoed an already-authorized one
— and every existing `DENIED`-aware query (`list_pending_approval`,
`find_resumable_action`, `has_unresolved_action`) already correctly
excludes it, with zero further changes needed. The two cases stay
distinguishable in the audit trail via a new, distinct event type
(`policy.auto_allowed_rejected`, mirroring U50's own
`policy.auto_allowed_executed` vs `policy.granted` split) rather than a
new status value.

**Where the branch belongs — resolved by contrast with U50's `grant`.**
U50 put its branch in the `service`-layer HTTP handler, because
`grant`'s two paths do genuinely different *shapes* of work (skip
straight to `authorize`+`execute` vs. call `approval::grant` first).
`reject`'s two paths do the *same* shape of work either way — transition
to `DENIED`, audit it — just from a different starting status and with
a different audit event type. So the branch lives entirely inside
`core::policy::approval::reject`, and **`service`'s `reject` HTTP
handler needed zero changes** — it already just calls
`approval::reject(...)` unconditionally.

`reject` now reads the row first (`get_action`, the identical pattern
`authorize` already used), validates its status is `PendingApproval`
or `AutoAllowed` (else a real `PolicyError::WrongStatus`, matching
`authorize`'s exact error shape), transitions it to `Denied` with
`check_not_expired` true only for the `PendingApproval` case (mirroring
`authorize`'s own identical derivation — an `AUTO_ALLOWED` row never
carries an expiry, so this changes no real behavior, just mirrors
existing precedent), then audits under the appropriate event type.

**Console**: the Reject button is now always shown (removed the
`AUTO_ALLOWED`-only guard U50 added) — it's a real, working veto for
both statuses now. No new l10n keys.

## Verification (real, not simulated)

- `core/tests/policy_approval_lifecycle_test.rs` (3 new tests — no
  core-level `reject` coverage existed in this file before this unit,
  even for the pre-existing path): a baseline `PENDING_APPROVAL`
  rejection (real `DENIED` + `policy.rejected` audit event); the new
  `AUTO_ALLOWED` veto (real `DENIED` + the new, distinct
  `policy.auto_allowed_rejected` audit event); rejecting an
  already-`DENIED` row fails, not a panic.
- `service/tests/policy_endpoints.rs` (2 new tests): a real `AUTO_
  ALLOWED` row rejected over real HTTP, real `DENIED` row, real
  distinct audit event (read directly from `audit_events`, the same
  technique U50 used since `actions::execute`'s own trailing audit
  event would otherwise make "latest event" ambiguous — not needed
  here since `reject` never calls `execute`, but kept for direct,
  unambiguous proof); rejecting an already-`EXECUTED` row is still a
  real `400`, proving the CAS's status check is genuinely enforced,
  not bypassed by the new branch.
- `console/test/policy_inbox_screen_test.dart`: the existing
  `AUTO_ALLOWED` test's assertion flipped from `findsNothing` to
  `findsOneWidget` for the Reject button — a real behavior change
  asserted, not a stale check silently dropped; new test proving
  tapping Reject on an `AUTO_ALLOWED` row posts the real request and
  the row disappears.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `flutter analyze`/`flutter test` clean (65 console tests, 1
  updated + 1 new). Both grep gates clean (no new SQL). Schema-drift
  clean (no `contracts` change). `cargo deny check`/`cargo audit`
  clean.

## Consequences

- Every `AUTO_ALLOWED` row now has a genuine, symmetric pair of
  human-triggered outcomes — run it or veto it — closing the last gap
  U50/ADR 0055 explicitly deferred.
- `approval::grant`'s own logic is completely untouched; `reject`'s new
  behavior only ever widens what it *accepts*, never changes what it
  *does* for the pre-existing `PENDING_APPROVAL` path.
- This closes the last remaining item identified in this session's
  fresh, full ADR re-audit's small/medium-scoped candidate list. The
  one remaining large item — FR-DBSEC-001's entirely unimplemented
  DBMS security-event detection (repeated auth failure, permission
  changes, unusual client addresses, config changes) — still needs its
  own dedicated scoping/plan-mode pass before it's a well-justified
  next unit.
