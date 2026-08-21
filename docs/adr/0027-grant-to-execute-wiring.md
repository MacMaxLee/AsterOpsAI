# 0027 — Service: Grant → Execute Wiring

## Status

Accepted (unit U22).

## Context

ADR 0018 (U13) and ADR 0019 (U14) both explicitly forward-referenced
this exact gap. `service/src/api/v1/policy.rs`'s own module doc said it
outright: *"Never calls `authorize()`/`execute()`... actually running an
approved action is real, separate work for a future unit."* Approving a
pending action in the console's Policy inbox has been cosmetic since
U13 shipped — it moved a status column and nothing else.

`core::actions::execute` and `core::policy::approval::authorize` (both
U7/U8) were already fully built and fully tested at the `core` level,
proven end-to-end in `core/tests/actions_executor_pipeline_test.rs`
(propose → grant → authorize → execute against a real spawned process).
This unit is pure wiring — zero changes to `core::actions`,
`core::policy`, or `core::tuning`.

## Decision

**`AppState` gained three fields, built once at startup, not
per-request**: `action_context` (wraps `platform`, already held),
`action_registry` (`Arc<ActionTypeRegistry>`), `protected_resources`
(`Arc<ProtectedResourceRegistry>`). The registry-building logic
(`build_action_registry`) was placed in `service`'s own library crate
(`service/src/actions.rs`), not as a private function in `main.rs` — a
real discovery mid-implementation: `service/tests/policy_endpoints.rs`
needs the *identical* registry `main.rs` uses (an empty one silently
breaks every grant-related test with `UnknownActionType`), and a
binary-only private function can't be shared with the test crate. One
real implementation, two callers, not a duplicated copy.

**The registry is seeded with the two real, shipped `ActionKind`
entries** (`host.set_process_priority`, `host.set_process_cpu_affinity`)
under the exact string keys `core::tuning::candidates.rs` already
proposes tuning actions under (confirmed by grep, not guessed) —
Linux-gated, matching `core::actions::host`'s own module gate. On other
platforms the registry is empty and a grant attempt fails with a real
`UnknownActionType`, mapped to a real `501 Unsupported` (see below), not
a crash or a faked success.

**`protected_resources` ships genuinely empty.** FR-POL-006's
protection stage runs for real on every `execute()` call; there is
simply no config surface anywhere in this project yet for actually
protecting a named resource. Not this unit's job to invent one.

**Error mapping needed two real fixes discovered only by testing for
real**, not designed up front:

1. `PolicyError::UnknownActionType` had no explicit arm in
   `policy_error_to_api` — it fell into the generic `other => Internal`
   branch, meaning a non-Linux `service` (or any grant against an
   unregistered action type) would report a `500` for what's actually a
   `501`-shaped, honestly-nameable condition ("this server has no
   implementation of this action type"). Added an explicit arm.
2. `ActionError::CapabilityUnavailable` was first mapped to `ApiError::
   Unsupported` (a reasonable-looking guess) — real testing (killing the
   target process before granting) proved this wrong: `Set
   ProcessCpuAffinityAction::check_capability()` directly probes the
   live target as *part of* its capability check
   (`platform.get_process_cpu_affinity(pid)`), so a vanished target
   surfaces through the exact same `CapabilityUnavailable` variant as a
   genuine "this platform can't do this at all" case — `core`'s own
   error type doesn't distinguish the two. By the time `execute()` is
   running, the action type *was* found in the registry, so this is
   really a request-specific failure, not a server-wide capability gap
   — remapped to `ApiError::BadRequest`. `ApiError::Unsupported` is now
   reserved for the one case that's genuinely about the *server*, not
   the request: `PolicyError::UnknownActionType`.

**Response contract stays `ApiResponse<()>`, unchanged. No console
changes.** If `execute()` fails *after* `authorize()` already moved the
row past `APPROVED` (to `EXECUTING` → `FAILED`, recorded by `execute()`
's own already-tested internal `fail()` helper), the HTTP response still
reports `success: false` with the real reason — deliberately more
honest than reporting "granted" when the action didn't actually apply.
U16's existing console error-handling path (`_postForSuccess`/
`_decodeEnvelopeSuccess`, already tested: "tapping Reject with a real
400 shows an inline error and keeps the row") already covers this
correctly.

## Real bug fixed: a placeholder parameter hash

`service/tests/policy_endpoints.rs`'s own `insert_pending` helper used a
literal placeholder `parameters_hash: "hash".to_string()` instead of a
real computed hash. Harmless before this unit (`authorize()` was never
called), but `authorize()` recomputes the real SHA-256 hash of the
supplied parameters and compares — every existing grant test would have
failed with `ApprovalMismatch` the moment this unit shipped. Fixed using
the real, already-`pub` `ai_ops_core::policy::hash::
compute_parameters_hash`, and every test's fixture now carries
parameters that actually match its own action type
(`{"cpus": [0]}` for `host.set_process_cpu_affinity`).

## Real bug fixed: a fictional target PID

The same helper also targeted a nonexistent `pid: 999_999`. Once grant
genuinely calls `execute()`, every action against that target now
legitimately fails (`capture_previous_state`/`check_capability` both
need a live process) — the tests were unknowingly relying on the fact
that nothing downstream of `grant` ever looked at the target for real.
Every grant-path test now spawns a genuine, throwaway `sleep 300` child
process (`SpawnedTarget`, this test file's own small helper — computing
real `start_time_ticks` by reading `/proc/[pid]/stat` field 22
directly, the same real field `core::actions::host::
ProcessTargetVerifier` itself reads, reimplemented locally since that
function is `pub(crate)` to `core`) and reaps/kills it in `Drop` — no
leaked processes confirmed after every test run.

## Verification (real, not simulated)

- `granting_a_reversible_action_really_executes_it`: grants a real
  `host.set_process_cpu_affinity` proposal via a real HTTP request
  against a real spawned process, then independently re-queries the
  *actual* OS-level CPU affinity mask via `platform.
  get_process_cpu_affinity` and asserts it's now exactly `{0}` — proof
  the real syscall ran, not just that a status column changed.
- `a_grant_whose_target_vanished_before_execution_reports_a_real_
  failure`: kills the real target before granting; asserts the real
  HTTP response reports `success: false` with a real `400` and the row
  lands on `FAILED` — never silently `EXECUTED`, never stuck.
- Full existing `service`/`console` suites stay green, including U16's
  own real grant test on the console side (unchanged, still passing,
  now backed by genuine execution).

## Consequences

- Approving something in the Policy inbox now genuinely does what it
  claims. ADR 0018/0019's forward-referenced gap is closed.
- **No live path exists yet to *create* a pending action through the
  running product.** `service` has no tuning-plan-start endpoint or any
  other proposal-creation surface — the only way a real
  `ASK_BEFORE_CHANGES`-mode candidate could land in the inbox today is
  `core::tuning`'s own pipeline, which `service` never calls. Deferred,
  explicitly, as a separate future unit — not pulled into this one's
  scope.
- `host.set_process_priority` is registered but has no test coverage in
  this unit (ADR 0013's own `CAP_SYS_NICE` rollback asymmetry made
  `host.set_process_cpu_affinity` the honest, unprivileged choice for a
  from-scratch integration test) — its wiring is real and reachable,
  just not independently re-proven here on top of `core`'s own existing
  coverage.
