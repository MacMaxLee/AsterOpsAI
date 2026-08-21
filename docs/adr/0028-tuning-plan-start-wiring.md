# 0028 — Service: Tuning Plan Start Wiring

## Status

Accepted (unit U23).

## Context

ADR 0027 (U22) closed "approving does something real" but named the
gap it deliberately left open: *"No live path exists yet to create a
pending action through the running product."* `core::tuning::
start_plan` (U10) — propose → evaluate → [auto-execute or leave
pending] — was already fully built and core-tested, but nothing in
`service` ever called it.

This unit wires `POST /api/v1/tuning/start`. It is pure wiring — the
real logic (`start_plan`, `evaluate`, `decide`) is untouched — but
testing that wiring for real, against the real, currently-registered
action types, surfaced a real architectural finding that reshaped this
unit's own "capstone" test partway through implementation (see below).

## Decision

**Request type stays local to `service`, matching the existing
`GrantRequest`/`RejectRequest` precedent.** `StartPlanRequest { pid,
start_time_ticks, resource_name, profile, mode, requested_by }`, with
small local `WireTuningProfile`/`WireAutomationMode` enums mapping 1:1
onto `core::tuning`'s own — a bad wire value is a real, honest `400`
from serde, not a silently-defaulted one. `TargetIdentity` is always
constructed as `Process`; the wire contract never offers `DbSession`,
since `core::tuning::build_candidates` itself rejects that target kind
outright.

**The response *does* get a real `contracts` type** — unlike grant/
reject, which return `()`. `contracts::TuningPlanOutcome { plan_id,
status, candidates: Vec<TuningCandidateOutcome> }` mirrors `core`'s
own `TuningOutcome`/`CandidateOutcome` field-for-field, schema-emitted
(`tuning_plan_outcome.schema.json` +
`envelope_tuning_plan_outcome.schema.json`). This is deliberate: a
`RECOMMEND_ONLY` plan's candidates are never persisted anywhere else a
human could see them later — this response is the only place they're
ever visible at all.

**`Environment::Development` is hardcoded at the point of use**,
matching U20's `dbms_config.rs` precedent exactly, for the identical
reason: no config surface for "what environment is this deployment"
exists anywhere in this project yet. Flagged plainly in both the
handler and this ADR, not silently invented as a new `AppState` field.

**`target_verifier()`, `policy_error_to_api`, and `action_error_to_api`
moved from `policy.rs` into the shared `service::actions` module**
(created in U22 for `build_action_registry`), and a new
`tuning_error_to_api` was added alongside them. `tuning.rs` needed all
three; duplicating them would have immediately diverged from the real
fixes U22 already made (e.g. `CapabilityUnavailable` → `400`, not
`501`). One real implementation, every caller — a small, in-scope
refactor, no behavior change (confirmed: `policy_endpoints.rs`'s full
suite still passes unchanged after the move).

**`TuningError` mapping**: `PlanAlreadyInFlight`/`UnsupportedTarget` →
`BadRequest`; `PlanNotFound` → `NotFound` (logged as `error!`, since
reaching this from a plan `start_plan` itself just inserted would be a
genuine bug, not a client mistake); `CapabilityUnavailable` →
`BadRequest` (same reasoning as U22's identically-named `ActionError`
variant); `Policy`/`Action` → delegate to the existing mappers.

**`SpawnedTarget` (a real throwaway process + its real
`/proc/[pid]/stat`-derived `start_time_ticks`) moved from
`policy_endpoints.rs`'s own private copy into `service/tests/support/
mod.rs`**, shared by both `policy_endpoints.rs` and
`tuning_endpoints.rs` now — avoiding a second, divergent copy of the
same real fixture (Cargo integration test binaries in different
*files* of the same package can share code via `mod support;`, unlike
different *packages*, which genuinely can't — ADR 0025's own
constraint doesn't apply here).

**`TuningProfile::Custom` and `DbSession` targets stay out of
scope**, named rather than silently half-supported: `Custom` needs its
own `DesiredState` wire payload (priority + affinity), a real,
separable follow-on; `core` itself already rejects `DbSession` for
tuning, so the wire contract simply never offers it.

## Real discovery: `ASK_BEFORE_CHANGES` cannot reach the Policy inbox today

The approved plan's capstone test assumed starting a plan in
`ASK_BEFORE_CHANGES` mode against a real spawned process would produce
`PENDING_APPROVAL` candidates, visible in `/api/v1/policy/pending`,
grantable through U22's real endpoint. Running that test for real
failed immediately:

```
left: "AUTO_ALLOWED_PENDING"
right: "PENDING_APPROVAL"
```

Reading `core::policy::evaluate` and `core::policy::risk::decide`
directly (not guessing) explains why: the policy *decision*
(`AutoAllow` / `RequireApproval` / `Deny`) is a pure function of
`(risk_level, environment)` — `AutomationMode` plays no part in it at
all. `AutomationMode` only governs what happens *after* `evaluate()`
returns: whether an already-`AutoAllow`ed candidate gets auto-executed
immediately (`AUTO_LOW_RISK`, if reversible and with a recorded
improved-outcome history), or held back (`ASK_BEFORE_CHANGES`, and
`AUTO_LOW_RISK` when the auto-execute gate isn't met) — landing on the
outcome string `AUTO_ALLOWED_PENDING`, not `PENDING_APPROVAL`.

Both currently-registered action types
(`host.set_process_priority`, `host.set_process_cpu_affinity`) are
`RiskLevel::Low` (confirmed by direct read of both `risk_level()`
implementations), and `decide(Low, Development)` is `AutoAllow`
(`Low` only requires approval in `Production`). So under this unit's
hardcoded `Environment::Development`, **`start_plan` can never produce
a `PENDING_APPROVAL` row for any action type registered today** —
regardless of `AutomationMode`. This exactly matches `core`'s own
pre-existing test coverage
(`tuning_plan_recommend_and_ask_test.rs::ask_before_changes_proposes_
for_real_but_never_authorizes`, which asserts the identical
`AUTO_ALLOWED_PENDING` outcome) — the *service* layer wasn't buggy;
the *plan*'s assumption about which outcome was reachable was wrong.

The consequence compounds: an `AUTO_ALLOWED_PENDING` row is real,
audited, and has a real `row_id` in the `actions` table with status
`AUTO_ALLOWED` — but it is genuinely unreachable from every existing
HTTP surface. `list_pending_approval` (backing `/api/v1/policy/
pending`) filters strictly `status = 'PENDING_APPROVAL'`, so the row
never appears there. U22's grant endpoint's first step,
`approval::grant`, has its own compare-and-swap requiring the row's
*current* status to already be `PENDING_APPROVAL`; granting an
`AUTO_ALLOWED` row fails that CAS and returns a real, correct `400`
(`"action {id} is not in the expected status: expected
PENDING_APPROVAL, actual AUTO_ALLOWED"`) — even though
`approval::authorize` itself would happily accept an `AUTO_ALLOWED`
row directly, if anything ever called it.

This unit's own `tuning.rs` module doc originally claimed *"the whole
propose -> inbox -> grant -> execute loop, reachable live over
HTTP"* — that claim was written before this was tested for real, and
was wrong. It has been corrected in the module doc, and the capstone
test was rewritten to assert the real, discovered behavior instead of
the originally-assumed one: `ask_before_changes_creates_a_real_row_
the_existing_inbox_cannot_yet_reach` proves, over real HTTP, that (a)
the row is real and audited, (b) it is genuinely absent from the
inbox, (c) granting it is a real, correct `400` naming its actual
status, and (d) the live target's CPU affinity never changed. This is
a real, honest boundary — not a workaround.

**Not fixed in this unit** (would require either a change to
`core::policy`/`core::repository` — expanding `list_pending_approval`
and/or the grant CAS to also recognize `AUTO_ALLOWED_PENDING` rows —
or a config surface for `Environment`, or a Medium/High-risk action
type; all out of this unit's pure-wiring scope per its own FORBIDDEN
list). The full propose → inbox → grant → execute loop over live HTTP
remains unreachable for *any* action type registered today, and stays
open as a named gap for a future unit.

## Verification (real, not simulated)

- `recommend_only_returns_real_candidates_and_touches_nothing_else`:
  `BATTERY_SAVER` against a real freshly-spawned process (default
  Normal priority, full CPU affinity) genuinely differs on both
  fields, so both real candidates come back `RECOMMENDED` with no
  `row_id`; confirms nothing appears in the inbox afterward.
- `ask_before_changes_creates_a_real_row_the_existing_inbox_cannot_
  yet_reach`: see above — the real, corrected capstone.
- `starting_a_second_plan_for_an_in_flight_target_is_a_real_bad_
  request`: a real `IN_FLIGHT` row is inserted directly (the same real
  partial `UNIQUE` index migrations/V11 enforces), then a real HTTP
  start-plan call against the same target gets a real `400` naming
  "already in flight" — proving the endpoint surfaces the existing
  repository-level guard correctly, not re-testing the guard's own
  mechanism (already covered by `core`'s
  `tuning_plan_concurrency_test.rs`).
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift clean
  (`tuning_plan_outcome`/`envelope_tuning_plan_outcome` committed);
  `cargo deny check`/`cargo audit` clean; no leaked processes after
  the full suite (`ps aux` checked for stray `sleep 300`/postgres).

## Consequences

- Starting a tuning plan is now reachable live over HTTP, and its
  `RECOMMEND_ONLY` output is now the only place a suggestion is ever
  visible to a human at all — a real, standalone product capability on
  its own.
- The propose → inbox → grant → execute loop, this unit's original
  intended flagship achievement, is confirmed **not yet reachable**
  end-to-end over HTTP for any currently-registered action type — a
  real, load-bearing finding, documented rather than quietly patched
  around. Closing it needs one of: a Medium/High-risk action type, a
  real `Environment` config surface, or teaching the inbox/grant path
  about `AUTO_ALLOWED_PENDING` — any of which is real, separable
  future work.
