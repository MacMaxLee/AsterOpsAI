# 0002 — Contract-First Wire Types via `contracts` + schemars, CI-Enforced Drift

## Status

Accepted (unit U0).

## Context

The console (unit U3) and the core service must never silently diverge on
what a wire type looks like — a hand-maintained duplicate DTO on the client
side is exactly the kind of drift that produces confusing runtime failures
months later, discovered only when a field is renamed on one side and not
the other. TRS §16 requires the console's models to be generated, not
hand-written.

## Decision

Every wire type is defined exactly once, in the `contracts` crate, deriving
both `serde::{Serialize, Deserialize}` and `schemars::JsonSchema`. The
`emit-schemas` binary (`cargo run -p contracts --bin emit-schemas`) renders
each type to `/schemas/*.schema.json`. These files are committed. CI's
schema-drift job re-runs `emit-schemas` and fails the build if the working
tree changes as a result — the committed schemas and what `contracts` would
actually produce can never quietly disagree.

## Consequences

- Any change to a `contracts` type must be accompanied, in the same PR, by
  re-running `scripts/emit-schemas.sh` and committing the result.
- Unit U3's console codegen consumes `/schemas/*.schema.json` as its sole
  input for generating client models; it has no reason to ever hand-write a
  DTO.
- Generic contract types (`Envelope<T>`, `SelfMetricValue<T>`) get schemars'
  auto-generated, type-parameter-suffixed schema names (e.g.
  `Envelope_for_HealthResponse`) unless a manual `schema_name()` override is
  added later — acceptable for U0's small type set; revisit only if U3's
  codegen finds the generated names awkward to work with.
- `contracts` has zero workspace-internal dependencies by design, so nothing
  needed for schema generation can accidentally pull in telemetry, policy, or
  DBMS logic.
