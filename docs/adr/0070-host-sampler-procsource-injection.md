# 0070 — Injectable `ProcSource` for `HostTelemetrySampler`

## Status

Accepted (unit U65).

## Context

ADR 0069 left FR-SYS-003 ("the core degrades sampling frequency under
host resource pressure") open as a genuine structural gap, not a quick
test: `service::telemetry::sampler::HostTelemetrySampler` hardcoded
`source: RealProcSource` rather than accepting an injectable
`&dyn ProcSource` the way `core::telemetry`'s own parse functions
(`parse_cpu_snapshot`, `parse_memory_snapshot`, …) already do. Without
that, no test could drive a controlled `CpuPressure::High`/`Critical`
fixture through `tick()` to observe the real
`NORMAL_INTERVAL` → `BACKED_OFF_INTERVAL` switch — only the live
machine's actual, unpredictable CPU load could trigger it.

## Decision

**`HostTelemetrySampler.source` is now `Box<dyn platform::linux::ProcSource>`**,
not the concrete `RealProcSource`. `HostTelemetrySampler::new()` (the
only constructor `spawn()` — real production code — ever calls) is now a
thin wrapper: `Self::with_source(Box::new(RealProcSource))`.
`with_source` is the new test-only injection point, documented as such,
mirroring unit U62's own `self_metrics::spawn_with_interval` precedent
for the identical "add a `with_*` constructor, keep production code
calling the plain one" shape. `linux_impl` (previously a private
submodule of `sampler.rs`, making `HostTelemetrySampler` unreachable
from any test outside `sampler.rs` itself) is now `pub`, which is also
what surfaced a real, previously-invisible `clippy::new_without_default`
lint on `new()` — fixed with a real `Default` impl, not suppressed.

**`rust_core/service/tests/host_sampler_backoff_test.rs`** (new): a
`ScriptedCpuPressureSource` implementing the real `ProcSource` trait,
built from the *same real fixture* `core::telemetry::cpu`'s own tests
already use (`tests/fixtures/proc/cpu-saturated/{before,after}`), loaded
via `include_str!` rather than duplicated as inline literals — not a
new, second copy of "what saturated CPU looks like." It can't reuse
`core`'s own `FixtureProcSource` (fixed to one phase, `core`-crate-
internal, `#[cfg(test)]`-gated, so unreachable from `service`'s tests at
all), so it's a small, purpose-built source that switches from `before`
to `after` content after the first `proc/stat` read — sufficient because
`parse_cpu_snapshot` always reads `proc/stat` first, exactly once per
tick, before `proc/loadavg`/`proc/uptime`. Every other path returns
`NotFound`; `tick()`'s own existing per-section error handling already
degrades each of those to `Unavailable` independently rather than
failing the whole tick, and this test only asserts on the CPU-driven
backoff, not on every subsystem rendering.

## Verification (real, not simulated)

- New test asserts both ticks: the first (no previous sample) is
  `CpuPressure::Normal` at `NORMAL_INTERVAL` (`core::telemetry::cpu`'s
  own documented no-baseline-yet fallback), the second (a real
  before→after delta from the saturated fixture) is `High`/`Critical`
  at `BACKED_OFF_INTERVAL` — the actual claim FR-SYS-003 makes,
  end-to-end through the real `tick()` method, not a unit test of an
  extracted pure function.
- Rerun 3× locally to confirm determinism (no timing dependency — the
  fixture switch is driven by a call counter, not a clock).
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `scripts/check-schema-drift.sh`, both grep gates, and
  `scripts/check-fr-id-traceability.sh` all pass. No console changes.

## Consequences

- `docs/traceability-matrix.md`: **59 OK, 1 NEEDS-VERIFICATION, 3
  MISSING** (of 63 total FR-IDs) — the only remaining
  `NEEDS-VERIFICATION` row is FR-CORR-003, ADR 0069's own named,
  deliberate ADR-0017-documented deferral (not attempted here, for the
  same reason ADR 0069 gave: no test can honestly prove a promotion
  that hasn't happened).
- `HostTelemetrySampler::with_source` and the now-`pub` `linux_impl`
  module are real, if narrow, additions to `service`'s public API
  surface — a future reader adding a new telemetry-sampling behavior
  under host-pressure conditions should extend this same test, not
  build parallel fixture-injection machinery.
