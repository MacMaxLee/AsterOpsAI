# 0014 — Benchmark Statistical Methodology: Mann-Whitney Choice, Missing Confounders, and Deterministic-Sampler Testing

## Status

Accepted (unit U9).

## Context

TRS §34 explicitly binds itself to this unit ("PERF-1... binding on unit
U9"): a baseline window of >=60s/>=30 samples gated on coefficient of
variation, an identical-shape post-change window, a distribution
comparison (bootstrap CI on the median, *or* Mann-Whitney U), and a
four-value verdict (IMPROVED/REGRESSED/INCONCLUSIVE/UNSTABLE) with
INCONCLUSIVE as the honest default. Several concrete decisions came up
that neither SRS/TRS pins nor any earlier unit resolved, and real testing
surfaced two genuine bugs (one in a test's own sampler design, one in the
test's own expectations about a real OS privilege constraint) — recorded
here so the reasoning and the fixes survive past this session.

## Decision

**Mann-Whitney U, not bootstrap-CI.** TRS §34 permits either. No
statistics crate exists anywhere in this workspace's dependency tree, and
this codebase's own style (`analysis::score`'s hand-rolled weighted sums,
`repository::audit`'s hand-rolled hash chain) favors direct arithmetic
over a new dependency for one well-specified algorithm. Mann-Whitney U is
deterministic — no RNG needed, unlike a percentile bootstrap, which would
also have made the verdict itself non-reproducible for the same input
data. `benchmark::stats::mann_whitney_u` hand-rolls the rank-sum U
statistic with tie correction and a normal-approximation p-value
(Abramowitz-Stegun 7.1.26 for `erf`), verified against two independent
hand-worked cases — one tie-free (`u_statistic` exactly 0, hand-computed
z/p), one with two size-3 tie groups exercising the tie-correction term
specifically (`benchmark_stats_test.rs`) — not just internal
self-consistency.

**`MetricDirection` is a necessary addition TRS never names.** A verdict
can't distinguish IMPROVED from REGRESSED without knowing whether the
benchmarked metric is one you want lower (CPU utilization, latency) or
higher (throughput). `resolve()` takes it explicitly; nothing infers it.

**`analysis::HostVerdict.score` cannot serve as the benchmarked metric.**
`classify_host` collapses an entire window into one `u8` per call — a
single point value, exactly what FR-BENCH-001 forbids treating as a
verdict basis. `benchmark::MetricSampler` is its own, decoupled concept:
a per-sample numeric value the orchestrator polls directly, never routed
through `core::analysis`'s window-level classification or persisted
telemetry at all.

**Three of TRS §34's five named confounders stay honestly `None`.**
Thermal state and AC/battery transitions have no sensor data source on any
platform (the same gap `analysis`'s ADR 0010 already documented for
THERMAL/POWER host bottlenecks); "other active tuning plans" has no data
source either, since TRS §36's tuning-plan concept doesn't exist yet
(confirmed downstream of this unit — it needs U9's own `Improved` value to
exist first). `Confounders` carries all five fields per TRS §34's own
list; only `process_count_delta` (a real `/proc` directory-entry count
taken before/after the run) and `sample_gaps` (real, detected from actual
poll timestamps) are ever populated.

**TRS §35's "CRITICAL audit event" is encoded via `event_type`, not a new
schema field.** `NewAuditEvent` (U2) has no severity column. Adding one
this far downstream, for one caller, would be a disproportionate schema
change; `"benchmark.rollback_failed"` is a distinct, greppable event type
a UI can treat specially without a new column.

**Real bug found during testing: a flat scripted sampler can leak
baseline-flavored values into the post-change window.** The first version
of `benchmark_run_test.rs`'s regression scenario used one `Vec<f64>` split
at a fixed position (15 baseline values, then 15 post-change values),
assuming the baseline window would consume exactly the first 15. It
didn't — `collect_window` only consumes as many samples as its own
`min_window`/`min_samples` conditions actually need, which isn't precisely
predictable from outside. The leftover baseline values bled into the start
of the post-change window, corrupting its coefficient of variation and
producing a spurious `Unstable` verdict instead of the expected
`Regressed`. Fixed by `PhaseAwareScriptedSampler`
(`core/tests/benchmark/common/mod.rs`): the phase transition is keyed off
*real* live state (whether the target process's priority/affinity has
actually changed yet), not a guessed call count — real wiring, real
timing, only the *magnitude* within each phase is scripted. This matches
this unit's own testing-strategy rationale (real pipeline, deterministic
measured values) more precisely than the first attempt did.

**Real discovery during testing, not a bug: rollback of a priority-
lowering action can't be asserted to succeed unprivileged.** ADR 0013
already documents that restoring a *lowered* nice value needs
`CAP_SYS_NICE` this sandbox's test user doesn't have. The end-to-end
"REGRESSED triggers a real rollback" test originally used
`host.set_process_priority` and asserted `rolled_back == true`
unconditionally — which correctly failed, for the same real, already-
documented reason. Rather than weaken the assertion to tolerate two
outcomes, the test was switched to `host.set_process_cpu_affinity`
(no such privilege asymmetry — U8's own test already proves a full
unprivileged round trip), letting it assert a genuinely successful real
rollback instead of a "maybe" — a stronger, more honest test than either
the original broken assertion or a loosened one would have been.

**A benchmark run only proceeds on auto-allowed (low-risk) actions.**
If `evaluate()` returns `PendingApproval` or `Denied`, `run_benchmark`
returns an error rather than silently granting the approval itself or
blocking indefinitely for a human. Benchmarking is meant for reversible,
low-risk actions (SRS FR-TUNE-003's own framing); auto-approving on an
unattended caller's behalf would be a real policy bypass, not a
convenience.

## Consequences

- A future Tuning Engine unit (TRS §36) that wants to run benchmarks
  automatically must itself only submit actions whose registered risk
  level auto-allows in the target environment — `run_benchmark` won't do
  that filtering for it.
- `process_count_delta`/`sample_gaps` are the only two confounders with
  real teeth today; a future unit adding thermal/power sensor exposure or
  a tuning-plan registry should populate the other three here rather than
  leaving them silently `None` forever.
- `HostCpuUtilizationSampler` is a deliberately minimal, self-contained
  `/proc/stat` aggregate reader — not a reuse of `core::telemetry::cpu`'s
  fuller parser (which also tracks per-core detail, ctxt, and intr, none
  of which this sampler needs) — the same class of judgment call U8's own
  field-22 extraction note already established.
