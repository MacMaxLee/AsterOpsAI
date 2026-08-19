# 0006 — Memory and CPU Pressure Classification Thresholds

## Status

Accepted (unit U1).

## Context

SRS FR-MEM-001 requires a deterministic memory pressure classification
(NORMAL/ELEVATED/HIGH/CRITICAL) but pins no exact numbers. U1's requirement 8
(back the host telemetry sampler off to a 5s interval under host CPU
pressure) implies a parallel CPU pressure classification that SRS §6 doesn't
spell out as explicitly as memory's. Neither SRS nor TRS specifies exact
thresholds for either — this is deliberately an ADR-level judgment call, not
a silently hardcoded constant a future unit would have to reverse-engineer.

## Decision

Both classifiers live in one module, `core::telemetry::pressure`, with named
constants directly above the functions that use them.

**Memory** — primary signal is `available_ratio = available_bytes /
total_bytes` (the host total, or the resolved cgroup v2 ceiling when
containerized):

| Tier | Condition |
|---|---|
| NORMAL | `available_ratio > 0.30` |
| ELEVATED | `available_ratio > 0.15` |
| HIGH | `available_ratio > 0.05` |
| CRITICAL | `available_ratio <= 0.05` |

Secondary signal, `swap_used_ratio = swap_used_bytes / swap_total_bytes`
(skipped entirely when no swap is configured), can only **raise** the final
tier, never lower it — a machine with plenty of free RAM but swap nearly
exhausted is still under real pressure:

| Tier | Condition |
|---|---|
| NORMAL | `swap_used_ratio < 0.25` |
| ELEVATED | `swap_used_ratio < 0.50` |
| HIGH | `swap_used_ratio < 0.85` |
| CRITICAL | `swap_used_ratio >= 0.85` |

Final tier = `max(memory_tier, swap_tier)`.

**CPU** — a single signal, `aggregate_utilization_percent`:

| Tier | Condition |
|---|---|
| NORMAL | `< 70.0` |
| ELEVATED | `< 85.0` |
| HIGH | `< 95.0` |
| CRITICAL | `>= 95.0` |

Boundary semantics are asymmetric by design and match the tables above
exactly (memory boundaries are `<=`/`>` at the CRITICAL/next-tier edge;
swap and CPU boundaries are `>=`/`<`) — the unit tests in `pressure.rs`
assert every edge value lands in the tier the table above says it should,
so this asymmetry is pinned by tests, not just prose.

## Consequences

- Unit U1's sampler back-off (5s tick interval under HIGH/CRITICAL CPU
  pressure) is driven directly by `classify_cpu_pressure`'s output.
- A future unit that wants to tune these numbers (e.g. once real-world data
  from unit U9's benchmarking suggests better thresholds) does so via a new
  ADR, per the "later unit deviates via a new ADR, doesn't edit a merged
  one" convention already established in `CLAUDE.md` — not a silent constant
  edit in `pressure.rs`.
- Memory pressure classification currently mixes cgroup-scoped
  total/available with host-scoped swap accounting when containerized
  (cgroup v2's own `memory.swap.current`/`memory.swap.max` isn't read yet —
  see the comment in `core::telemetry::memory`); this ADR's thresholds apply
  to whatever ratio is actually computed today, and will need no change if
  that gap is closed later — only the units feeding the ratio would change.
