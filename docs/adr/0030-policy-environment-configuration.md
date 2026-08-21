# 0030 — Service: Policy Environment Configuration

## Status

Accepted (unit U25).

## Context

ADR 0028 (U23) named three ways to close the gap it discovered: the
propose → inbox → grant → execute loop was unreachable over HTTP for any
action type registered today, because `core::policy::risk::decide`
always resolves `RiskLevel::Low` (both currently-registered action
types) to `AutoAllow` under the hardcoded `Environment::Development`. Of
the three named options — a Medium/High-risk action type, a real
`Environment` config surface, or teaching the inbox/grant path about
`AUTO_ALLOWED_PENDING` — this unit does the second: the smallest,
most contained, and the one that (as a side effect) finally delivers the
capstone U23 originally set out to prove.

## Scope-critical distinction

Two structurally unrelated types are both named `Environment` and were
both hardcoded to `Development` in `service` before this unit:

- `core::dbms::connection_metadata::Environment` — a DB-connection
  metadata label (`service/src/dbms_config.rs:67`, unit U20).
- `core::policy::risk::Environment` — the risk-decision input
  (`service/src/api/v1/tuning.rs`, unit U23), the one ADR 0028 actually
  named.

This unit is about the second one only. Grepping every `Environment::`
use in `service/src` confirmed `tuning.rs` was the **only** live call
site consuming `policy::Environment` — U22's grant handler
(`policy.rs`) never calls `evaluate()` again after a plan is proposed, so
it never needs an `Environment` at all. `dbms_config.rs`'s own hardcoding
is untouched, a deliberately separate future decision if ever revisited.

## Decision

**New env var `ASTEROPS_POLICY_ENVIRONMENT`** (`development` / `staging`
/ `production`, lowercase — matching `ASTEROPS_DB_SSLMODE`'s own
convention), resolved once at startup by the new `service::policy_config`
module — a near-exact structural mirror of `dbms_config.rs`: a pure,
unit-testable `resolve_policy_environment_from(raw: Option<String>) ->
Environment` plus a thin real-env-var wrapper. Unset, empty, or an
unrecognized value all fall back to `Environment::Development` — the
identical "unparsable falls back to the default, never errors" judgment
call `dbms_config::parse_tls_mode` already makes and already has test
coverage for, and the same value this call site was hardcoded to before
this unit, so an unconfigured deployment's behavior is unchanged.

**`AppState` gained `pub policy_environment: Environment`** (`Copy`, no
`Arc` needed). `AppState::new()`'s arity grew to 8, the same way every
prior unit that added state did (U20's `dbms_adapter`, U22's action
trio) — no builder introduced, consistent with existing precedent.
`main.rs` resolves it once, alongside the existing `dbms_config::
resolve_db_connection()` call, and passes it straight through.
`tuning.rs`'s `start` handler now reads `state.policy_environment`
instead of the hardcoded literal; the stale "no config surface exists"
doc comments (both the U23 module-level one and the inline one at the
call site) were corrected to describe the real config that now exists.

**Every `service` test file that builds `AppState` directly** (all nine
of them) needed the new positional argument — all pass
`Environment::Development` unconditionally except `tuning_endpoints.rs`,
which gained a second entry point, `build_app_with_environment(repo,
environment)`, with the existing `build_app` delegating to it at
`Development` — used by exactly one new test.

## The capstone, finally real

`environment_production_makes_ask_before_changes_really_reach_the_
inbox_and_grantable` (`tuning_endpoints.rs`): builds the app with
`Environment::Production`, starts an `ASK_BEFORE_CHANGES` plan
(`BATTERY_SAVER`) against a real spawned process. `core::policy::risk::
decide(Low, Production)` is `RequireApproval` (unlike `Development`), so
both real candidates come back `PENDING_APPROVAL` — not
`AUTO_ALLOWED_PENDING`, as the otherwise-identical `Development`-default
test right above it in the same file asserts. The row is genuinely
visible in `/api/v1/policy/pending`; granting it via U22's real endpoint
succeeds; the target's real CPU affinity actually changes afterward.
This is the exact loop ADR 0028 documented as unreachable — now proven
reachable, live over real HTTP, for the first time in this project's
history, given the right deployment configuration.

## Verification (real, not simulated)

- `policy_config`'s own 6 unit tests: unset/empty/unrecognized all fall
  back to `Development`; each of the three valid lowercase values is
  recognized correctly.
- The new capstone integration test, described above — real HTTP, real
  spawned process, real CPU affinity change confirmed independently via
  `platform.get_process_cpu_affinity`.
- Every existing test across all 9 `service` test files still passes
  unchanged (all pinned to `Development`, matching prior behavior
  exactly) — this unit adds a config surface, it does not change any
  existing default behavior.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean; both grep gates clean; schema-drift a real no-op (nothing here
  touches `contracts`); `cargo deny check`/`cargo audit` clean; no
  leaked processes.

## Consequences

- A deployment can now genuinely opt into `Production`'s stricter policy
  posture, and doing so has an immediate, real, verified effect: the
  full propose → inbox → grant → execute loop becomes reachable over
  HTTP for both currently-registered action types.
- The remaining two gap-closing options ADR 0028 named — a Medium/
  High-risk action type, and teaching the inbox/grant path about
  `AUTO_ALLOWED_PENDING` — remain open, real, separable future units;
  this one doesn't reduce their value, since the default deployment
  posture (`Development`, unconfigured) still exhibits the original gap.
- `dbms_config.rs`'s own, structurally unrelated `Environment::
  Development` hardcoding remains, named but untouched — a distinct
  future decision if ever revisited.
