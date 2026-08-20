# 0013 — Real Action Surface: Process Priority & CPU Affinity

## Status

Accepted (unit U8).

## Context

U7 shipped the policy/approval/executor framework with deliberately zero
real `ActionKind`/`TargetVerifier` implementations. Every repo artifact
pointing at "unit U8" agreed real actions arrive here, but none of them
named which ones — until TRS §38 (Windows platform notes) was read in
full: *"...`SetPriorityClass` + `SetProcessAffinityMask` for **the two U8
actions**."* That's the one unambiguous scope statement for this unit
anywhere in the spec (SRS §21/TRS §34's benchmark methodology is
explicitly bound to unit U9 by its own text and by ADR 0006; TRS §33's
`analyze_table` DB-action example and TRS §36's tuning engine carry no
unit binding and need a `TargetIdentity` model extension of their own).
U8 therefore ships two real, reversible, host-level actions — adjust a
process's scheduling priority, adjust its CPU affinity — through U7's
executor pipeline, for real, on Linux.

## Decision

**`ActionTypeEntry.construct` and `policy::approval::authorize` both gain
a parameter — a flagged, necessary change to U7's just-shipped API.** A
bare `fn` pointer (`fn(&ActionRequest) -> Box<dyn ActionKind>`) can't
capture a shared dependency like a live `PlatformAdapter`. `ActionContext
{ platform: Arc<dyn PlatformAdapter> }` (`core::actions::context`) is the
new thing `construct` receives; `authorize` gained the parameter it needed
to thread through to `construct`. Every U7 call site — production and
test — was updated in this unit; nothing was left silently broken.
`construct` also became fallible (`Result<Box<dyn ActionKind>, String>`,
not the plain `Box<dyn ActionKind>` U7 shipped): real construction can
fail in a way `validate_parameters` structurally cannot catch, since
`validate_parameters` only ever sees the parameter JSON, never the
target — an action type registered for `Process` targets receiving a
`DbSession` one is exactly this case, and it needed a real `Err` path
(`PolicyError::ConstructionFailed`), not an `unreachable!()` or a panic.

**`ProcessPriority` is a 5-level enum shaped after Windows' priority
*classes*** (Idle/BelowNormal/Normal/AboveNormal/High), not a raw nice
value, so the same type carries one cross-platform meaning even though
only Linux has a real implementation this unit. **Realtime is deliberately
excluded** — a documented, flagged safety scope-down: a realtime
scheduling class can starve the rest of the system in a way a five-level
priority-class model shouldn't casually expose, and TRS §38 doesn't name
it either.

**The *getters* read `/proc`, not `getpriority(2)`/raw `sched_getaffinity`
parsing.** `getpriority(2)`'s return value is genuinely ambiguous on its
own — `-1` is both a legitimate nice value and the traditional error
sentinel, and POSIX requires clearing `errno` before the call and checking
it after to disambiguate. Rather than take on that errno-clearing
discipline, `get_priority` reads `/proc/[pid]/stat`'s own field 19 (the
kernel's own textual nice value) and `get_cpu_affinity` reads
`/proc/[pid]/status`'s `Cpus_allowed_list` — real, unambiguous, and
consistent with `core::telemetry`'s own longstanding preference for
`/proc` parsing over syscalls wherever both give the same answer. Only the
*setters* (`setpriority`/`sched_setaffinity`) need a real `unsafe` syscall,
each with an unambiguous `0`/`-1` return — no errno dance needed there.

**Real bug-shaped discovery during testing, not a code bug: priority
lowering is asymmetrically reversible for an unprivileged caller.** Linux
only ever lets an unprivileged process *raise* its own nice value (lower
its priority) relative to whatever the value currently is — never lower it
again, even to undo a change it made itself; that direction needs
`CAP_SYS_NICE` regardless of who set the current value or why. So
`Normal -> BelowNormal` applies cleanly, but rolling `BelowNormal ->
Normal` back needs the same privilege a fresh `Normal -> AboveNormal`
attempt would, and fails the identical honest way
(`CapabilityError::PermissionRequired` → `ActionError::Rollback`). This
was found by writing the "happy path always rolls back" test first,
watching it fail for a *correct* reason, and rewriting the test
(`rollback_of_a_priority_lowering_action_fails_cleanly_when_unprivileged`,
`actions_host_priority_affinity_test.rs`) to assert the real constraint
instead of a false assumption — the same "does the test fail for the
right reason" discipline this codebase applies everywhere else. This is a
real, standing limitation of `SetProcessPriorityAction`, not something
this unit's code should (or safely could) paper over: any successful
priority-*lowering* apply by an unprivileged actor is not reversible by
that same actor. CPU affinity has no such asymmetry — an owning user can
freely widen or narrow their own process's affinity mask in either
direction, and `set_process_cpu_affinity_end_to_end_with_rollback` proves
a full round trip.

**`risk_level()` is `Low` for both action types, not parameter-varying.**
Both mutate one process's own scheduling, both are bounded in blast
radius, and both are captured/reversible in the common case (the priority
exception above is a privilege gap, not a design choice this unit could
route around by scoring `AboveNormal`/`High` differently — those already
fail cleanly at `apply()` for an unprivileged caller regardless of risk
tier). A documented judgment call, not something derived from a
requirement.

**Windows/macOS stay `Unsupported`-stubbed**, matching
`self_process_metrics`'s own existing U12 deferral exactly — telemetry
itself isn't implemented on those platforms yet, and this sandbox can't
test either OS for real. The Windows stub's doc comment cites TRS §38's
exact API names (`SetPriorityClass`/`SetProcessAffinityMask`) for that
unit's eventual implementer; the macOS stub notes `setpriority(2)` (same
POSIX call as Linux) for priority and flags that macOS has no direct
`sched_setaffinity` equivalent (real affinity control there needs
`thread_policy_set`/taskpolicy-style tagging instead).

## Consequences

- `core::actions::host` is registered nowhere in shipped `service` code
  yet (matching every prior unit's incremental-layering precedent) — a
  future wiring unit registers `set_process_priority_entry()`/
  `set_process_cpu_affinity_entry()` into a live `ActionTypeRegistry`,
  gated to `cfg!(target_os = "linux")` until Windows/macOS get real
  implementations.
- `SetProcessPriorityAction`'s rollback limitation (privilege-gated,
  direction-dependent) should be surfaced to an operator/caller before
  this action type is ever exposed through an approval UI — FR-BENCH-003's
  "reversible or explicitly labeled otherwise before applied" language
  anticipates exactly this kind of conditional reversibility; a future
  wiring unit should label a priority-lowering request as such rather than
  imply unconditional reversibility.
- DB-side actions (`analyze_table` etc., TRS §33), benchmark methodology
  (SRS §21/TRS §34, U9), and the tuning engine (TRS §36) remain
  unaddressed by this unit, each for the reasons given above — not
  overlooked, deliberately deferred.
