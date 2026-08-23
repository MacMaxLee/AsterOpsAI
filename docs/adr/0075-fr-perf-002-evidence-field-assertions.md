# 0075 — FR-PERF-002: Assert Evidence Fields by Value, Not Just Presence

## Status

Accepted (unit U69).

## Context

ADR 0069 tagged FR-PERF-002 as `OK` but named a real caveat:
`evidence_window_spans_the_supplied_history` only asserted window
bounds and non-emptiness — no test anywhere asserted the `metric` name
or `observed`/`threshold` field *values* by name, leaving the full
claim ("evidence carries a real metric name, observed value, and
threshold, not placeholders") only partially proven.

## Decision

Extended the existing test rather than adding a new one — it already
constructs the exact scenario needed (`rows_with_pressure(5, "HIGH",
"NORMAL")`: 5/5 samples CPU `HIGH`, so the first evidence item is
deterministically the CPU domain's own real fraction). Added
assertions on `evidence.metric` (`"cpu_pressure_sustained_fraction"`),
`evidence.observed` (`1.0`, the real `crossed_count / sample_count`),
`evidence.threshold` (`SUSTAINED_FRACTION`, the real constant, not a
hardcoded duplicate of its value), and `evidence.unit` (`None`).

## Verification (real, not simulated)

- All 14 tests in `analysis_host_classification_test.rs` still pass.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean.
- `docs/traceability-matrix.md` regenerated: no change — FR-PERF-002
  was already `OK`; this closes the specific caveat ADR 0069 named
  under that same tag, without changing the matrix's status.

## Consequences

- None beyond closing the named caveat — no new gaps found while doing
  this.
