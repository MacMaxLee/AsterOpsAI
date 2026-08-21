# 0035 — Core: Persist `previous_state`, Closing ADR 0033's Named Gap

## Status

Accepted (unit U30).

## Context

ADR 0033 (U28) named a real, deliberate limitation when it wired
`rollback_by_row_id`: `previous_state` was never persisted anywhere
(`migrations/V4__actions.sql`/`V9__policy_approval_columns.sql` have no
column for it), so a row-id-based rollback could only reconstruct it as
`Value::Null`. Proven safe at the time — both `host.set_process_
priority`/`host.set_process_cpu_affinity`'s own `rollback()`
implementations reject a non-matching shape cleanly — but never
*correct*: `host.*` rollback-by-row-id could fail safely, but could
never actually restore anything. This unit closes that gap for real.

Confirmed by direct read before changing anything: `execute()`'s Stage
6 (`core/src/actions/executor.rs`) already computes the real
`previous_state` via `action.capture_previous_state()`, well before
`apply()` ever runs. The real value has always been in hand at Stage
9's own `transition_action` call — it was simply never written anywhere
durable. This is a small, surgical addition to an already-correct
pipeline, not new logic, and not a change to `rollback()` itself.

## Decision

**New migration `V13__actions_previous_state.sql`**: `ALTER TABLE
actions ADD COLUMN previous_state_json TEXT;` — nullable, since
existing rows (and any row executed before this migration lands on a
running deployment) simply won't have one.

**`TransitionPatch`/`PolicyActionRow`** each gain `previous_state_json:
Option<String>` (`core/src/repository/models.rs`). `TransitionPatch`
already derives `Default`, and every one of its four construction
sites already used `..Default::default()`, so this is additive — the
other three callers (`executor::fail`, `approval::grant`, `rollback::
rollback`'s own new-row transition) are unaffected.

**`core/src/repository/policy.rs`**: `SELECT_COLUMNS`/`row_from_sqlite`
gain the new column; `transition()`'s two SQL branches each gain a
`previous_state_json = COALESCE(?N, previous_state_json)` clause —
the identical pattern every other patchable column already uses.

**`core/src/actions/executor.rs`**: Stage 9 now serializes the
already-captured `previous_state` and includes it in the same
`TransitionPatch` that already records `executed_at`/`result_json` —
the one real behavior change in this unit. A serialization failure
(realistically unreachable for an already-valid `Value`, but handled
honestly rather than assumed away) goes through the same `fail()` path
every other stage already uses.

**`core/src/actions/rollback.rs`**: `rollback_by_row_id` now parses
`row.previous_state_json` when present, falling back to `Value::Null`
only for the one real, narrower remaining case — a row that predates
this migration. That row still gets exactly today's pre-U30
safe-failure behavior (or, for `security.suspend_process`, whose own
`rollback()` ignores `previous_state` entirely, still resumes
correctly regardless) — never an error, never silently wrong.

## Real test consequence: one existing test's premise flipped

`core/tests/actions_rollback_by_row_id_test.rs`'s own
`rollback_by_row_id_of_an_affinity_change_fails_safely_without_the_
real_previous_state` (U28) existed specifically *because*
`previous_state` didn't exist yet. Rewritten — not deleted — as
`rollback_by_row_id_of_an_affinity_change_really_restores_the_real_
previous_value`: reads the real original affinity mask before
anything runs, executes a real change, rolls it back through
`rollback_by_row_id` alone (no `Executed` handle carried over), and
asserts the live mask is genuinely back to the original. The
non-reversible-guard test (`host.set_process_priority`) is untouched —
that guard fires before `previous_state` is ever consulted.

## Verification (real, not simulated)

- `core/tests/actions_rollback_by_row_id_test.rs` (5 tests, 1 rewritten
  as above): the affinity-restore capstone proves real, historical
  state genuinely comes back — not just "doesn't crash."
- `service/tests/policy_endpoints.rs`
  (`granting_an_affinity_change_then_rolling_it_back_over_http_really_
  restores_it`, NEW): the same proof over real HTTP — grant a real
  affinity change, roll it back in a genuinely separate HTTP call,
  confirm the live mask is back to its original value. The first proof
  this real restore works end-to-end over HTTP, not just at the `core`
  level.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift a real no-op (no wire
  types touched); `cargo deny check`/`cargo audit` clean; no leaked
  processes; the new migration confirmed picked up (the documented
  `embed_migrations!` staleness gotcha in `core/src/repository/
  migrations.rs` was proactively avoided by touching that file before
  the first build).

## Consequences

- `host.set_process_cpu_affinity` rollback-by-row-id now genuinely
  works, not just fails safely — ADR 0033's own named gap is closed.
- `host.set_process_priority` rollback-by-row-id remains refused
  outright by the pre-flight `reversible` guard (ADR 0013's own
  `CAP_SYS_NICE` asymmetry finding) — unrelated to and unaffected by
  this unit, a genuinely separate real constraint.
- Rows executed before this migration lands on a real, running
  deployment keep exactly today's `Value::Null`/safe-failure behavior
  for `host.*` (their `previous_state` is genuinely gone, not
  recoverable after the fact) — named, not silently pretended away.
