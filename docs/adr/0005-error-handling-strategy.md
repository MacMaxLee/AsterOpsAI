# 0005 — Typed Per-Domain Errors, Converted to a Closed API Error Enum at the Boundary

## Status

Accepted (unit U0).

## Context

TRS §8 requires every fallible boundary to return `Result`, not panic, and
requires the wire-facing error to be a closed type, never a bare string a
client would have to pattern-match by message text. At the same time,
different layers (platform syscalls, future DB queries, future policy
checks) have meaningfully different failure shapes that shouldn't all be
flattened into one giant enum up front.

## Decision

Each crate defines its own `thiserror`-based domain error type close to
where the failure actually happens (`platform::CapabilityError` today; more
follow as later units add persistence, policy, and DBMS layers). Only
`service`'s handlers convert a domain error into `contracts::ApiError` — the
small, closed, wire-facing enum from ADR 0004's sibling discussion — at the
point of building the HTTP response. No domain error type is ever serialized
directly onto the wire.

Two mechanical consequences of the lint requirements are recorded here so
future units don't have to rediscover them:

- Cargo's `unsafe_code = "forbid"` lint, once set in `[workspace.lints]`,
  cannot be downgraded to `allow` in a dependent crate's own `[lints]`
  table — `forbid` is non-overridable even in child scopes. `platform`
  therefore does *not* use `[lints] workspace = true`; it defines its own
  local `unsafe_code = "deny"` table instead, and marks individual FFI call
  sites `#[allow(unsafe_code)]`.
- Cargo's `[lints]` table has no `cfg(test)` concept, so denying
  `clippy::unwrap_used`/`clippy::expect_used` "outside tests" (requirement 8)
  needs a matching `#![cfg_attr(test, allow(clippy::unwrap_used,
  clippy::expect_used))]` at the top of every crate root.

## Consequences

- A new API-visible failure mode is always a deliberate, named
  `ApiError` variant addition — never a leaked internal type serialized by
  accident.
- Tests keep ergonomic `.unwrap()`/`.expect()`; production code paths do not.
- Every crate's `lib.rs`/`main.rs` carries the `cfg_attr` line above; a new
  crate added by a later unit must include it too.
