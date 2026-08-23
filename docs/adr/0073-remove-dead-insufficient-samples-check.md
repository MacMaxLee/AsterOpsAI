# 0073 — Remove Dead `InsufficientSamples` Check

## Status

Accepted (unit U67).

## Context

ADR 0069 named `BenchmarkError::InsufficientSamples` (the "minimum...
sample count" half of FR-BENCH-001) as a small, well-scoped, real
untested path: "defined and used but never triggered in any test."
Investigating it to add that test found the actual situation was
different: `run_benchmark`'s check —
`if baseline.samples.len() < request.config.min_samples { return
Err(BenchmarkError::InsufficientSamples { .. }) }` — is not merely
untested, it's **unreachable**. `collect_window`'s only exit is its
`break`, gated on `samples.len() >= config.min_samples`; every
`Ok(Window)` it returns already satisfies that bound by construction.
Confirmed further: `InsufficientSamples` was defined once, checked
once, and matched nowhere else in the codebase — no service-level error
mapping, no test, nothing depending on it existing.

Presented to the user as a real fork: remove the dead check as honest
cleanup, or give it real teeth by adding a max-duration timeout to
`collect_window` (a genuine, separate robustness gap — the loop
currently has no upper bound at all if a technically-succeeding sampler
is simply slow). The user chose removal — a timeout policy is a real
behavior change deserving its own deliberate design, not a side effect
of closing a traceability-matrix row.

## Decision

- Removed the dead check in `run_benchmark` and the now-fully-unused
  `InsufficientSamples { got, need }` variant from `BenchmarkError`.
- Added a doc comment on `collect_window` stating the confirmed
  invariant directly, so a future reader doesn't have to re-derive it:
  every `Ok(Window)` already satisfies `samples.len() >= min_samples`.
- Did **not** add a max-duration timeout — named as a real, separate,
  deliberately-deferred concern, not attempted here.

## Verification (real, not simulated)

- Confirmed directly (grep, not assumed) that `InsufficientSamples` had
  no other reference anywhere in the workspace before removing it —
  removing a variant that's still matched elsewhere would have been a
  compile error, not silently wrong, but confirmed the search first
  rather than relying on the compiler alone to catch it.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `benchmark_run_test.rs`'s existing two tests (which do reach
  `collect_window` and `run_benchmark` for real) still pass unchanged —
  this removal doesn't touch anything they were exercising.

## Consequences

- `docs/traceability-matrix.md` is unaffected — FR-BENCH-001 was
  already `OK` via `baseline_unstable_never_applies_the_action`, which
  covers a different real claim (the baseline-stability abort path, not
  the sample-count check this ADR removes).
- A future unit that wants `collect_window` to bound its own worst-case
  runtime should treat that as new, deliberate scope — not resurrect
  this specific dead variant, since its "got"/"need" shape was never
  designed around a timeout scenario (it reported a sample *count*
  short-fall, not an elapsed-time short-fall).
