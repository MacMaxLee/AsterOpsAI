# 0019 — Service Wiring: Tuning Plan History, Raw JSON Passthrough, and the History/Inbox Ordering Split

## Status

Accepted (unit U14).

## Context

U13 shipped the policy approval inbox (`v0.14-service-policy-inbox`),
establishing the real pattern for wiring a `core` domain into `service`:
a new `contracts` module, a `list_*` repository query, `service/src/
api/v1/<domain>.rs`, real `tower::ServiceExt::oneshot` integration
tests. `repository::tuning` had the same gap `repository::policy` had
before U13 — only `get_by_id`, no list query — so U10's real
`tuning_plans` rows have never been visible to anything either. This
unit applies U13's exact pattern to tuning plan history.

## Decision

**`target_identity_json` and `candidates_json` are passed through raw,
not parsed — a real, deliberate difference from `PendingActionSummary`'s
parsed `resource_kind`/`resource_name` fields.** `TargetIdentity` has two
structurally different variants (`Process{pid, start_time_ticks}` /
`DbSession{backend_pid, backend_start}`) with no shared "name" field the
way `core::policy::ResourceDescriptor` has — there's no clean "kind+name"
projection to parse it into without inventing a new `contracts` wire
enum mirroring `TargetIdentity`, which is real, separate design work a
list endpoint doesn't need to take on. `candidates_json` (each candidate
action's outcome) has the same problem one level further:
`core::tuning::CandidateOutcome` isn't a `contracts` type either. Both
fields are typed `String` on the wire and asserted, in
`tuning_endpoints.rs`, to be genuinely valid, parseable JSON — a real
passthrough contract, not an excuse to skip validating the shape.

**The list is capped at a real, explicit `LIMIT 100`, most-recent-first
— the opposite ordering from U13's inbox, and worth stating why.**
`tuning_plans` has no retention policy (unlike telemetry rollups, which
roll up and prune on a schedule), so an uncapped query would grow
without bound as plans accumulate. `policy::list_pending_approval`
orders oldest-first because an inbox is meant to drain from the front;
`tuning::list_recent` orders newest-first because a history view is read
from the most recent event backward — two different real use cases,
not an accidental inconsistency. `tuning_endpoints.rs`'s 105-row test
proves both the cap and the ordering for real, not by reading the SQL.

**No `POST` endpoint to start a plan over HTTP — the same scope cut
U13 made for `authorize()`/`execute()`.** `core::tuning::start_plan`
needs a real `TuningPipeline` (registry, context, verifier,
protected-resource registry, environment) to do anything beyond
`RecommendOnly`; wiring that into `AppState` is real, separate work,
deliberately left to a future unit rather than folded in here to keep
this one a pure, small, read-only slice.

## Consequences

- A future unit adding a `TargetIdentity` (or `CandidateOutcome`) wire
  type would let a future revision of `TuningPlanSummary` parse these
  fields properly instead of passing them through raw — this ADR
  doesn't preclude that, it just states why this unit didn't do it.
- A future unit wiring `POST /api/v1/tuning/plans` needs to add the real
  `ActionTypeRegistry`/`ProtectedResourceRegistry`/`ActionContext` to
  `AppState` this unit deliberately didn't — the same dependency U13's
  own ADR already named for a future grant-then-execute unit.
- Security-incident endpoints are the natural next unit, following this
  exact same now-twice-proven pattern.
