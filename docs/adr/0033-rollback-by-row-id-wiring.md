# 0033 — Core + Service: Rollback-by-Row-Id (Resume) Wiring

## Status

Accepted (unit U28).

## Context

A real, urgent safety gap, found via direct research after U27 shipped:
an operator could suspend a real process (real `SIGSTOP`) through the
console, but **nothing in the running product could resume it**.
`core::actions::rollback::rollback` (TRS §27 / SRS FR-BENCH-003) is
real, complete, and its call into `SuspendProcessAction::rollback()` →
`PlatformAdapter::resume_process` genuinely works — but it takes a
whole `Executed` struct (`core/src/actions/executor.rs`), whose fields
are all private with no public constructor. The only way to obtain one
was `execute()`'s own return value, inside the same call. Grepped: zero
references to `rollback`/`rollback_of` anywhere in `service/` or
`console/`. Every existing `core` test called `execute()` then
`rollback()` back-to-back in the same test function — nothing
reconstructed an `Executed` from a row looked up later, because
nothing could.

**This unit is not pure wiring** — the first `core`-logic change since
this session's wiring units began (U22 onward). Every prior unit could
stay inside `service` because `core` already exposed everything needed;
this one genuinely required a small, new `core::actions` API. Flagged
explicitly here, not silently expanded scope.

## Why this is provably safe, not a guess

`previous_state` was never persisted (`migrations/V4__actions.sql` has
no such column), so a row-id-based rollback can't recover the *real*
historical previous_state for `host.set_process_priority`/`host.
set_process_cpu_affinity`. Read both action types' own `rollback()`
implementations directly before deciding anything: `priority.rs` calls
`previous_state.as_str()` (fails cleanly — `ActionError::Rollback
("previous_state is not a string")` — on anything but a string);
`affinity.rs` calls `json_to_affinity` (fails cleanly — `"previous_state
is not a valid affinity mask"` — on anything without a real `cpus`
array). Reconstructing `previous_state` as `Value::Null` therefore
**fails safely and honestly** for both host actions — never a wrong or
silent rollback — while `SuspendProcessAction::rollback()` ignores
`previous_state` entirely (unconditional `resume_process`), so it's the
one action type this genuinely, correctly resumes. This isn't asserted
from reading code alone — a new core-level test proves each claim for
real (see Verification).

## Decision

**One new, small `core::actions` API.** `Executed::reconstruct(row_id,
target, resource, previous_state, action)`
(`core/src/actions/executor.rs`) is `pub(super)` — visible only to
`core::actions`'s own children, never a public constructor anyone else
could misuse. `core::actions::rollback::rollback_by_row_id(row_id,
registry, context, verifier, rolled_back_by, handle) -> Result<i64,
ActionError>` (`rollback.rs`):

1. Looks up the row (`repository::get_action`); missing →
   `PolicyError::NotFound` (existing variant, real reuse).
2. Requires `status == "EXECUTED"`; anything else →
   `PolicyError::WrongStatus{expected: "EXECUTED", actual}` — the same
   shape `approval::grant`'s own CAS failures already use.
3. **A new, deliberate safety guard**: refuses outright if the action
   type's `reversible` flag (unit U10 — "proven to round-trip without
   any privilege gap," previously used only to gate `AUTO_LOW_RISK`'s
   auto-execute eligibility) is `false` — `ActionError::
   CapabilityUnavailable("action type {action_type} is not
   reversible")`. `host.set_process_priority` is `reversible: false`
   (ADR 0013's own `CAP_SYS_NICE` asymmetry finding), so this is the
   *first* thing that stops a priority-change rollback attempt, before
   ever reaching `rollback()` itself and its own honest-but-less-clear
   failure. All three registered action types were already correctly
   flagged; this reuses that existing judgment, doesn't introduce a new
   one.
4. Reconstructs `TargetIdentity`/`ResourceDescriptor`/`parameters` from
   the row's own stored JSON — identical to what `approval::authorize`
   already does for grant, not new logic invented for this.
5. Calls the registry's `construct` fn, builds `Executed::reconstruct
   (..., Value::Null, ...)`, and calls the **existing, entirely
   unmodified** `rollback::rollback(&executed, ...)` — every real
   behavior (target re-verification, new audited row, `rollback_of`
   linkage, status transition) is untouched U11/U27-era code.

**Service**: a fourth policy route, `POST /api/v1/policy/{id}/rollback`
(`service/src/api/v1/policy.rs`), mirroring `grant`/`reject`'s exact
shape (`RollbackRequest { rolled_back_by }`, `ApiResponse<()>`). No new
error-mapping arms needed — `action_error_to_api` already handles
`Policy(...)`/`CapabilityUnavailable`/the generic fallback correctly
for every error this can produce, confirmed by reading it before
reusing it rather than assuming.

## Verification (real, not simulated)

- **`core/tests/actions_rollback_by_row_id_test.rs`** (NEW, 5 tests) —
  the first test file anywhere in this codebase to reconstruct a
  rollback from a row looked up *separately* from the original
  execution:
  - A real `security.suspend_process` proposal is proposed, evaluated,
    granted, authorized, and executed for real (real `SIGSTOP`,
    confirmed via `is_process_stopped`); a *separate* call to
    `rollback_by_row_id`, knowing only the row id, really resumes it
    (confirmed `is_process_stopped` is now `false`) — the capstone.
  - Rolling back a real, executed `host.set_process_cpu_affinity` row
    (reversible: true) fails with the real, honest "not a valid
    affinity mask" message — proves the safe-failure claim for real.
  - Rolling back a real, executed `host.set_process_priority` row
    (reversible: false) is refused outright with the new guard's real
    message, never reaching `rollback()` at all.
  - Rolling back a never-executed (`AUTO_ALLOWED`) row is a real
    `WrongStatus` error; rolling back a nonexistent row is a real
    `NotFound` error.
- **`service/tests/policy_endpoints.rs`** (3 new tests) — the same
  capstone over real HTTP: grant a real `security.suspend_process`
  proposal (target really stops), then a genuinely separate `POST
  /policy/{id}/rollback` call really resumes it, and the original
  row's own status stays exactly `EXECUTED` (the rollback is its own
  new, separately-audited row, not a mutation of the original). Plus a
  real `400` for a never-granted row and a real `404` for a
  nonexistent one.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift a real no-op (rollback
  returns `()`, no new contracts type); `cargo deny check`/`cargo
  audit` clean; no leaked processes.

## Consequences

- The full propose → grant → execute → **rollback** loop is now real
  and reachable over HTTP for the first time — closing the safety gap
  found after U27: a process suspended through the running product can
  now also be resumed through it.
- `host.*` rollback-by-row-id remains real but honestly non-functional
  (fails cleanly rather than silently wrong) until `previous_state` is
  actually persisted — named here as a real, separate future fix
  (a schema migration adding a `previous_state_json` column, plus
  `executor::execute` persisting it), not silently worked around.
- No console UI exists yet for this endpoint — a "resume" button on a
  suspended process is real, separate future work, deliberately out of
  this unit's scope (core+service only, matching the established
  alternating pattern).
