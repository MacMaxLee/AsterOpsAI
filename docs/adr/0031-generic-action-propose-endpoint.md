# 0031 — Service: Generic Action Propose Endpoint + Suspend-Process Wiring

## Status

Accepted (unit U26).

## Context

ADR 0028 (U23) named a Medium/High-risk action type as one of two
remaining ways to close the "propose → inbox → grant → execute loop is
unreachable" gap — U25 closed the third (`Environment` config) but only
under a non-default `Production` setting. Research for this unit found
the answer already exists: `core::security::response::
SuspendProcessAction` (`core/src/security/response.rs`, unit U11,
TRS §37 / SRS FR-SEC-004) is a real, fully-implemented,
fully-core-tested `RiskLevel::High` action — real `SIGSTOP`/`SIGCONT`
via `PlatformAdapter::suspend_process`/`resume_process`, reversible,
capability-complete on Linux — that was simply never registered in
`service::actions::build_action_registry()` and had no HTTP path to ever
be proposed at all. `TRS §37`'s other named example, `BLOCK_DEVICE`, is
explicitly deferred in that same file's own doc comment (needs a
`TargetIdentity::Device` variant that doesn't exist — see ADR 0016) —
untouched here, correctly so.

Because `decide(High, environment)` is `RequireApproval` in every
environment (confirmed by direct read of `core/src/policy/risk.rs`),
wiring this delivers a *stronger* version of U25's own capstone: the
full grant loop is now reachable under the default, unconfigured
`Development` environment too, with zero special deployment config.

## Decision

**A second, generic caller of `core::policy::{validate, evaluate}`,
not an extension of `/tuning/start`.** Those two functions are already
plain, non-tuning-specific — `core::tuning::plan.rs`'s own
`process_candidate` is just one caller. `/tuning/start` is built
specifically around `TuningProfile`'s desired-state diffing
(`core::tuning::build_candidates`), which has no notion of a security
response at all — extending it to also accept an arbitrary
`action_type`/`parameters` pair would have meant either bolting an
unrelated concept onto the tuning wire contract, or duplicating
`validate`/`evaluate` glue a second time. A new `POST /api/v1/
actions/propose` endpoint instead builds an `ActionRequest` directly
from client input and calls `validate()` then `evaluate()` itself —
reusable by `security.suspend_process` today and by any future
non-tuning action type, with zero changes to `core::policy` or
`core::tuning` logic (pure wiring, same discipline as every prior unit).

**Process-target-only**, matching `StartPlanRequest`'s own precedent
(U23) exactly: `TargetIdentity::Device`/`DbSession` aren't offered by
this wire contract — not silently unsupported, just not exposed yet.

**No auto-execute.** The new endpoint proposes only, mirroring
`ASK_BEFORE_CHANGES` semantics: it returns whatever real `PolicyOutcome`
resulted, and a row that lands `AUTO_ALLOWED`/`PENDING_APPROVAL` is
picked up by U22's *existing* grant endpoint — no new execute path, no
duplicated auto-execute logic to keep in sync with `core::tuning`'s own.
For `security.suspend_process` specifically this choice is moot in
practice: `RiskLevel::High` never auto-allows, so every real proposal is
`PENDING_APPROVAL`.

**Response gets a real `contracts` type** (unlike grant/reject):
`ActionProposalOutcome { row_id, status, approval_expires_at, reason }`
mirrors `core::policy::PolicyOutcome`'s three variants field-for-field —
the same "the response carries real, meaningful data" reasoning ADR
0028 already established for `TuningPlanOutcome`. Schema-emitted.

## Real, deliberate error-mapper extension

`/actions/propose` is the first caller anywhere in `service` to hand
raw, client-supplied JSON `parameters` straight to `core::policy::
validate()` — every earlier caller (`core::tuning`'s own
candidate-building) only ever validates internally-constructed,
pre-vetted parameters, so `PolicyError::InvalidParameters` had no
explicit arm in `service::actions::policy_error_to_api` and fell into
the generic `other => Internal` branch (would have reported a genuine
client mistake as a `500`). Added an explicit arm mapping it to a real
`400` carrying the actual validator's own rejection reason — the same
category of real, testing-driven fix ADR 0027 made twice for the grant
path. Verified with a real, malformed `host.set_process_cpu_affinity`
proposal (parameters missing `cpus` entirely).

## Verification (real, not simulated)

- `proposing_suspend_process_reaches_the_inbox_and_grant_really_stops_
  the_target`: proposes `security.suspend_process` against a real
  spawned process; the real row lands `PENDING_APPROVAL` under the
  *default* `Development` environment (no override needed, unlike U25's
  own capstone); confirmed visible in `/api/v1/policy/pending`; granted
  through U22's existing endpoint; the target process is confirmed
  **really stopped** afterward via `platform.is_process_stopped` (the
  same real check `core`'s own `SuspendProcessAction::verify()` uses).
  A second, independent, environment-config-free proof that the full
  propose → inbox → grant → execute loop is real.
- `proposing_an_unknown_action_type_is_a_real_unsupported`: unchanged
  `UnknownActionType` → `501` mapping, exercised through the new
  endpoint for the first time.
- `proposing_malformed_parameters_is_a_real_bad_request_not_an_internal_
  error`: proves the new `InvalidParameters` mapping.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift clean
  (`action_proposal_outcome`/envelope committed); `cargo deny
  check`/`cargo audit` clean; no leaked processes (a genuinely stopped
  test process is still reliably `SIGKILL`ed by `SpawnedTarget`'s
  existing `Drop`/`kill_and_reap` — confirmed, a stopped process still
  terminates on `SIGKILL`).

## Consequences

- `security.suspend_process` — real, tested, but completely unreachable
  since unit U11 — is now a genuine, live capability of the running
  product, reachable with zero special configuration.
- The generic propose endpoint is now the natural home for any future
  non-tuning action type (e.g. a future `BLOCK_DEVICE`, once ADR 0016's
  own deferral is revisited) — no new endpoint would be needed, just a
  registry entry.
- No console UI exists yet to call this endpoint — a "respond to a
  security incident by suspending its process" console screen is real,
  separate future work, deliberately out of this unit's scope (service
  -only, matching the U22/U23/U25 pattern).
