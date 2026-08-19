# 0004 — Capability Model: Typed, Reason-Carrying Enum, Single Registry

## Status

Accepted (unit U0).

## Context

SRS FR-CAP-001/002 forbid the server from ever standing in a fabricated `0`
or empty value for data it genuinely doesn't have — a metric can be missing
because it isn't implemented yet, because the platform doesn't expose it,
or because the current user lacks permission, and the console needs to
render these cases differently, never as "the value happens to be zero."

## Decision

`Capability` is a closed enum — `Supported`, `Limited { reason }`,
`Unavailable { reason }`, `PermissionRequired { reason }` — living in
`contracts`, not in `platform`, so the server and any future generated
documentation consume the identical type (TRS §5). Every non-`Supported`
variant structurally requires a reason string; there is no variant that
means "missing, no explanation." `default_capability_registry()` is the
single function both the `/health` endpoint and later units' documentation
generation call to know what this build claims to support.

`platform::CapabilityError` is a separate, internal, per-domain error type
(richer than the wire `Capability` enum needs to be) that crosses into
`contracts::Capability` only at the service boundary, via
`CapabilityError::to_capability()`.

## Consequences

- Adding a new capability family (e.g. `DbmsPostgresql` when unit U4 lands)
  is an additive change to `CapabilityFamily` and the registry function, not
  a new ad hoc "is this available" check scattered through handlers.
- Unit U0 ships every family as `Unavailable { reason: "not implemented
  until a later unit" }` — this is honest, not a placeholder to be
  embarrassed about: nothing is implemented yet.
- The console (unit U3) can render "unavailable, here's why" as a first-class
  UI state directly from this type, with no special-casing per metric.
