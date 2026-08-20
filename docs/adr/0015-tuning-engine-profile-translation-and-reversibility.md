# 0015 — Tuning Engine: Profile Translation, `reversible`, and the AUTO_LOW_RISK Gate

## Status

Accepted (unit U10).

## Context

SRS §22 (FR-TUNE-001..003) and TRS §36 describe a tuning engine: a
declarative profile translated into candidate actions, three automation
modes governing how far a plan goes on its own, a "one plan in flight per
target" concurrency rule, and an AUTO_LOW_RISK mode gated on reversibility
and a locally recorded history of improved outcomes. Neither document pins
"unit U10" literally, but ADR 0014 twice calls this "a future Tuning
Engine unit" and explicitly names TRS §36 as needing U9's own `Improved`
value to exist first — which it now does. Several concrete decisions came
up that neither SRS/TRS pins nor any earlier unit resolved.

## Decision

**Profile -> `DesiredState` is a small, explicit, hand-picked table, not a
formula.** `Balanced`/`Development` both mean Normal priority + full CPU
mask (kept as two named variants, not collapsed, since a future unit may
give `Development` its own meaning — e.g. disabling AUTO_LOW_RISK entirely
— without disturbing `Balanced`); `HighPerformance` means AboveNormal +
full mask; `BatterySaver` means BelowNormal + `{cpu 0}` specifically — a
deliberately simple choice, not a claim that CPU 0 is the most
power-efficient core on every real machine. `Custom(DesiredState)` lets a
caller bypass the table entirely. `std::thread::available_parallelism()`
gives a real "full CPU set" with no new `PlatformAdapter` method needed.

**`ActionTypeEntry` gains a real, structural `reversible: bool` field —
not a doc-comment convention.** ADR 0013 already proved
`host.set_process_priority`'s rollback can genuinely fail without
`CAP_SYS_NICE` when undoing a priority *lowering*: an unprivileged process
can raise its own nice value (lower its priority) freely but can never
lower it again, even to undo its own prior change. That action type
honestly cannot claim `reversible: true`. `host.set_process_cpu_affinity`
can: U8's own end-to-end test already proves a full unprivileged round
trip, with no analogous asymmetry. **Consequence, stated plainly rather
than discovered silently: with the current two-action catalog,
AUTO_LOW_RISK can only ever auto-apply CPU-affinity changes.** A future
unit adding a genuinely reversible action type widens this without any
change to the gate itself.

**FR-TUNE-002's concurrency guard is a real SQLite partial `UNIQUE` index,
not an in-memory lock.** `idx_tuning_plans_one_in_flight_per_target ON
tuning_plans (target_identity_json) WHERE status = 'IN_FLIGHT'`
(migrations/V11) is durable (survives a process restart, unlike a
`Mutex`), race-free through the existing single-writer thread (ADR 0007),
and needs no new synchronization primitive. `repository::tuning::
insert_tuning_plan` detects the constraint violation specifically (by
`ErrorCode::ConstraintViolation`, the only `UNIQUE`/`CHECK` index this
insert can hit) and maps it to a dedicated `RepositoryError::
TuningPlanAlreadyInFlight` -> `TuningError::PlanAlreadyInFlight`, rather
than letting it fall through the generic `Query` catch-all — the same
precise-error-discrimination precedent `repository::policy::transition`
already set for its own lifecycle states.
`tuning_plan_concurrency_test.rs` proves this with two genuinely
concurrent `start_plan` calls against the same real spawned target: both
send their `INSERT` as their very first `.await`, so the two commands
race through the writer thread's single channel and the real constraint
resolves the outcome — exactly one succeeds, the other is rejected
outright, never queued, regardless of which one the scheduler happened to
run first.

**FR-TUNE-003's AUTO_LOW_RISK gate is genuinely three-part, checked
together, not layered as independent filters.** A candidate only
auto-executes if it is `AutoAllowed` by `policy::evaluate` (which already
folds in risk level and environment — Low risk only auto-allows outside
Production) *and* its action type is `reversible` *and*
`history::has_improved_history` finds a real prior `IMPROVED`
`benchmark_runs` row for that action type. Anything failing any one of the
three falls back to `AskBeforeChanges` behavior (`AUTO_ALLOWED_PENDING` —
proposed and audited, left for a human) — never silently dropped.
`tuning_plan_auto_low_risk_test.rs` proves this is really about
`reversible`, not merely about history: it seeds a real `IMPROVED`
benchmark-history row for *both* `host.set_process_priority` and
`host.set_process_cpu_affinity`, then runs one AUTO_LOW_RISK plan
proposing both — only the affinity candidate auto-executes.

**`benchmark_runs` needed a real join it didn't have.**
U9's schema (migrations/V10) has no `action_type` column of its own — only
`action_id REFERENCES actions(id)`. `repository::benchmark::
query_improved_run_exists` adds `benchmark_runs JOIN actions ON
benchmark_runs.action_id = actions.id WHERE actions.action_type = ? AND
benchmark_runs.verdict = 'IMPROVED' AND benchmark_runs.rolled_back = 0` —
the `rolled_back = 0` clause matters: an `IMPROVED` verdict never triggers
a rollback (TRS §35's rollback triggers are `Regressed`/`Unstable` only),
so in practice it's always false for an `IMPROVED` row today, but the
clause makes the query's own intent — "a genuinely-kept improvement,"
not just a verdict string — explicit rather than accidental.

**`AskBeforeChanges` is deliberately scoped down, not a placeholder for
missing wiring.** No `service`/console unit exists yet to let a human
action a pending proposal. Rather than leave "ask" unimplemented, it means
exactly this today: `policy::evaluate` runs for real (a real proposed,
audited `actions` row exists — visible, queryable, eventually actionable
by a human once that wiring exists) but `start_plan` never calls
`approval::authorize`, regardless of what the policy decision would have
allowed. This is strictly more conservative than plain policy evaluation
on its own — the actual point of "ask" at this layer. `RecommendOnly` is
scoped further still: it never calls `policy::evaluate` at all, so it
creates zero `actions` rows — a pure suggestion, nothing audited, proven
by `tuning_plan_recommend_and_ask_test.rs` via a real `actions` row-count
comparison before/after.

**`start_plan` takes no Linux-only dependency directly.** The real
`TargetVerifier`/`ProtectedResourceRegistry` `AutoLowRisk` needs to
actually execute anything are supplied by the caller via
`TuningPipeline`, mirroring `BenchmarkPipeline`'s own precedent (U9) — the
same "unmodified U7/U8 pipeline," never a second way to reach `apply()`.
This keeps `core::tuning` itself platform-agnostic even though every
caller that wants AUTO_LOW_RISK to do anything supplies a real, Linux-only
verifier today.

**Failed candidate-building rejects the whole plan, not just skips it
silently.** If `build_candidates` errors (e.g. a `DbSession` target, which
this unit doesn't support), the `IN_FLIGHT` row is transitioned to
`REJECTED` with the error recorded in `candidates_json` before returning
`Err` — never left stuck `IN_FLIGHT` forever (which would permanently
block any future plan against that target under FR-TUNE-002's own
constraint) and never silently swallowed.

## Consequences

- A future unit adding a third real action type must set its
  `reversible` value deliberately and honestly — `false` is the safe
  default that this unit's own registrations already establish for
  anything not proven by a real round-trip test.
- A future console/service unit is what actually lets a human act on the
  `actions` rows `AskBeforeChanges`/`AUTO_ALLOWED_PENDING` leave behind;
  this unit only guarantees those rows exist and are audited, not that
  anyone can action them yet.
- `TuningPlanStatus::Rejected` today only ever means "candidate-building
  failed structurally" — it does not mean "every candidate was denied";
  a plan whose candidates are individually denied/pending still completes
  as `COMPLETED` (the plan's own attempt finished; each candidate's own
  outcome is recorded separately in `candidates_json`).
- `desired_state_for`'s table has no notion of per-process customization
  beyond `Custom` — a future unit wanting per-target profile overrides
  extends this table's caller, not its own shape.
