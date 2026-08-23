# 0067 — Benchmark-Run Rollback: A Narrowed Reading of FR-HIST-002

## Status

Accepted (unit U62).

## Context

ADR 0066 found FR-HIST-002 ("every persisted analytical row — performance,
security, action, benchmark — is immutable once written; corrections
happen via new rows, not updates") in real tension with
`repository::benchmark::mark_rolled_back`, which updates an existing
`benchmark_runs` row's `rolled_back`/`rollback_action_id` columns in place
rather than inserting a new row. That ADR explicitly left the question
open — "a genuine product/architecture question left for a human to weigh
in on" — rather than silently fixing or dismissing it. It also named three
concrete, ready-to-implement test-coverage gaps as future work:
FR-STO-001 (storage counter-reset path untested), FR-DB-005
(`pg_stat_statements` `Gated::Supported` happy path untested), and
FR-SYS-002 (`self_metrics` sampled values asserted-on nowhere).

`policy::transition_action` already has the identical shape for `actions`
rows: a compare-and-swap update gated on `expected_status`, used
throughout the policy lifecycle (approve, reject, execute) to transition
an action's status exactly once per valid state change, without ever
touching that row's own substantive fields (`action_type`,
`parameters_json`, `evidence_json`, …). No one has ever read that as a
violation of the same immutability requirement for `actions` rows — the
lifecycle status is understood as separate from the row's recorded
evidence.

## Decision

**FR-HIST-002 is narrowed, not overridden**: a persisted analytical row's
*substantive results* — for a benchmark run, its measured samples,
duration, and verdict — are genuinely immutable once written, exactly as
required. A row's own *lifecycle status* (here, `rolled_back` /
`rollback_action_id`) is a separate concern, and may transition exactly
once via a real compare-and-swap guard — mirroring
`policy::transition_action`'s own `expected_status`-gated pattern, not a
new precedent.

`repository::benchmark::mark_rolled_back` now issues
`UPDATE benchmark_runs SET rolled_back = 1, rollback_action_id = ?1 WHERE
id = ?2 AND rolled_back = 0`. A second call against an already-rolled-back
run affects zero rows; `must_get_by_id`'s own `?` already reports a
genuinely missing id, so an `affected == 0` result on a row that *is*
found can only mean the CAS guard rejected it — surfaced as the new
`RepositoryError::BenchmarkRunAlreadyRolledBack(id)`, mirroring
`TuningPlanAlreadyInFlight`'s own precedent of a real, durable rejection
rather than an in-memory check.

**Also closed, the three named test-coverage gaps from ADR 0066**:
- **FR-STO-001**: `telemetry::storage`'s counter-reset branch, previously
  exercised by no test (every existing storage test only ever passed
  `prev: None`) — a new `storage_counter_reset` test mirrors
  `network.rs::nic_counter_reset`'s exact two-phase (`before`/`after`
  fixture) shape.
- **FR-DB-005**: `query_stats`'s `Gated::Supported` happy path — the
  extension actually loaded — now exercised via
  `TestPostgres::start_with_extra_options`'s `shared_preload_libraries`
  (unit U51 built this; nothing had used it for this until now).
- **FR-SYS-002**: `self_metrics`'s sampled `self_cpu_percent`/
  `self_rss_bytes` values are now actually asserted on, via a new
  test-only `spawn_with_interval` override of the sampler's fixed
  interval — lets a test observe a real value in milliseconds rather than
  waiting a genuine 5 real seconds, the same `with_activity_thresholds`
  precedent unit U53 established for `PostgresAdapter`.

## Verification (real, not simulated)

- New `rust_core/core/tests/repository_benchmark_run_immutability_test.rs`:
  a first `mark_rolled_back` call succeeds against a real inserted run and
  a real `actions` row; a second call against the same, now-rolled-back
  run returns `RepositoryError::BenchmarkRunAlreadyRolledBack`, not a
  silent overwrite.
- Full workspace: `cargo test/clippy -D warnings/fmt --check` clean.
- CI green end-to-end on GitHub Actions after this unit's commit, including
  a separate, unrelated non-determinism bug this unit's own CI run
  surfaced and fixed in `scripts/generate-traceability-matrix.sh` (grep's
  filesystem- and locale-dependent match ordering was picking a different
  tie-broken module/unit column locally versus in CI).

## Consequences

- FR-HIST-002's immutability guarantee now has a documented, precedented
  boundary: substantive results vs. lifecycle status, matching how
  `actions` rows already work. A future reader hitting the same apparent
  tension for a different table (e.g. a hypothetical future in-place
  update) should look here first, not re-litigate from scratch.
- `docs/traceability-matrix.md` reflects FR-HIST-002, FR-STO-001,
  FR-DB-005, and FR-SYS-002 as `OK` rather than `NEEDS-VERIFICATION`.
- The remaining ADR-0066-named backlog (the ~27-FR-ID `NEEDS-VERIFICATION`
  set, TRS §20's AI-reachability dependency-graph check, TRS §46's RBAC
  docs-sync gate) is unchanged by this unit and still open.
