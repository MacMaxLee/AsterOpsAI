# 0018 — Service Wiring: Policy Approval Inbox, `reject`, and the First `contracts` Crossing Since U7

## Status

Accepted (unit U13).

## Context

Every unit since U4 has explicitly deferred "service/console wiring" as
out of scope, so U7's real `PENDING_APPROVAL` rows have sat in the
database with no way for a human to see or act on them. A research pass
confirmed `service`'s HTTP surface (axum, versioned `/api/v1`) is 100%
read-only telemetry, `AppState` has no policy/tuning/security wiring, and
`repository::policy`/`tuning`/`security` each expose only `get_by_id` —
no list query exists anywhere. This unit ships the smallest complete,
real slice that changes that: a policy approval inbox.

## Decision

**`policy::approval::reject` is a new, real addition — `grant` had no
counterpart.** SRS FR-POL-005 names three distinct outcomes needing an
audit record: "denials, expiries, and *rejections*." Only the first two
(both automatic — `evaluate()`'s own risk decision, and CAS expiry) had
code; a manual human rejection of a `PENDING_APPROVAL` row had no
function at all. `reject` mirrors `grant` exactly — the same
`PENDING_APPROVAL -> DENIED` compare-and-swap `grant` already uses for
`-> APPROVED`, just targeting `Denied` — but is audited under its own
`"policy.rejected"` event type, distinct from `evaluate()`'s own
`"policy.denied"`. Both rows end up `DENIED`; only the audit trail can
tell "the policy engine auto-denied this" apart from "a human rejected
this," which is exactly FR-POL-005's own three-way distinction, honored
at the point that actually matters. `reject` deliberately leaves
`approved_by` unset (same as an automatic denial) — the audit event is
this action's durable record of who did it, not a row column; adding a
`rejected_by` column would have been a real schema change for
information the audit chain already carries durably.

**This unit wires no `ActionTypeRegistry`/`ProtectedResourceRegistry`/
`ActionContext` into `AppState` at all.** Listing pending actions,
granting, and rejecting only ever touch the `actions` table's lifecycle
columns through the `RepositoryHandle` `AppState` already has — none of
them call `authorize()`/`execute()`. Wiring the real action registry so
an approved action can actually *run* is real, separate work, deliberately
left to a future unit rather than folded in here. This is also why the
scope stayed small enough to ship as one unit: no registry construction,
no per-action-type `check_capability` concerns, just three endpoints over
data that already exists.

**`contracts::policy::PendingActionSummary` is the first `contracts`
change since U7 — and the first time any unit has crossed the boundary
ADR 0017 flagged.** ADR 0017 noted `HostVerdict`/`DbHealthVerdict`/
`CorrelationResult` staying `core`-internal meant nothing could render
them over the wire yet; this unit is the first to actually add a wire
type for one of the four "framework" units (policy/tuning/security/
correlation) that have shipped since. It carries only what a listing view
needs — `id`, `created_at`, `action_type`, `risk_classification`,
`resource_kind`, `resource_name`, `requested_by`, `approval_expires_at`
— never the full `PolicyActionRow` (parameters, evidence, hashes are
internal lifecycle detail with no use in an inbox view). The
`PolicyActionRow -> PendingActionSummary` mapping (parsing
`resource_descriptor_json` via `core::policy::ResourceDescriptor`, which
is already `Deserialize`) lives in `service`, not `core` —
`contracts` has zero workspace-internal dependencies by design (ADR
0002), and `core` has no reason to know about `service`'s own wire
shapes. This unit's own DoD includes a real schema-drift regen
(`scripts/emit-schemas.sh`, two new committed schema files) — the first
time since U7 that drift is expected and resolved rather than absent.

**No auth/session system exists anywhere in this project yet — a real,
flagged limitation, not solved here.** `granted_by`/`rejected_by` come
directly from the POST body (`{"granted_by": "..."}` /
`{"rejected_by": "..."}`), the same "caller supplies the actor name"
shape `core/tests`' own `approval::grant` calls already use (e.g.
`"approver"`). A production deployment would need real authentication
before this endpoint is trustworthy — out of scope for this unit and
stated here plainly rather than silently assumed away.

**The inbox lists oldest-first (`ORDER BY created_at ASC`).** A real
approval inbox drains from the front — a deliberate choice, not an
arbitrary default the query happened to return.

## Consequences

- A future unit adding tuning-plan or security-incident endpoints
  follows this exact pattern: a new `contracts` module, a `list_*`
  repository query, `service/src/api/v1/<domain>.rs`, real `tower::
  ServiceExt::oneshot` integration tests — no new architecture to invent.
- A future unit wiring `authorize()`/`execute()` into `service` (so a
  granted action can actually run) needs to add the real
  `ActionTypeRegistry`/`ProtectedResourceRegistry`/`ActionContext` to
  `AppState` that this unit deliberately didn't.
- `PendingActionSummary`'s field list is a real, opinionated choice for
  a listing view; a future "get one action's full detail" endpoint would
  need its own, richer wire type, not an extension of this one.
- The console (Flutter — confirmed genuinely runnable in this
  environment via `/home/parallels/flutter/bin/flutter`) can now
  consume this endpoint for a real approval-inbox screen; that's a
  separate, real follow-up unit, not part of this one.
