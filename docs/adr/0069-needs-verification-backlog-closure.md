# 0069 — Closing the FR-ID NEEDS-VERIFICATION Backlog

## Status

Accepted (unit U64).

## Context

ADR 0066 named a ~27-FR-ID `NEEDS-VERIFICATION` backlog as "real, but a
much larger retroactive-tagging effort, out of this bounded unit's
scope." By the start of this unit, three of those had already closed
(FR-STO-001, FR-DB-005, FR-SYS-002, unit U62) and one and a new gate had
closed a TRS-level item (ADR 0068), leaving 23 real `NEEDS-VERIFICATION`
rows. This unit triages all 23.

## Decision

Each FR-ID got one of three honest outcomes — no row was closed by
weakening what "OK" means:

**13 already had a real, specific covering test — just uncited.** Each
got a precise one-line `/// SRS FR-XXX-NNN: <claim>.` doc comment
directly on the actual test that proves that specific claim (this
codebase's established convention, e.g. `telemetry/cpu.rs`'s own
`FR-CPU-001` tag), not a blanket file-level tag:
FR-MEM-001, FR-PRO-003, FR-PERF-002, FR-AI-005, FR-POL-002, FR-POL-004,
FR-BENCH-001, FR-BENCH-002, FR-BENCH-003, FR-TUNE-001, FR-CONSOLE-001,
FR-CAP-002 (console-rendering half). **FR-CORR-002** was the same
situation with a different root cause: `correlation_test.rs`'s own doc
comment already said `SRS FR-CORR-001..002`, but that range-shorthand
string doesn't contain the literal substring `FR-CORR-002` the
grep-based generator searches for — spelling both IDs out fixed it with
no new test needed. **FR-STO-002**'s existing doc comment was also found
to be stale (claiming untested when unit U62's `storage_counter_reset`
had already closed its `CounterReset` branch) and was corrected to name
only the branches still actually open.

**8 were genuine gaps, each closed with a real, narrowly-scoped test**:
- **FR-AI-003**: a real `serde_json::from_str` round-trip with an
  unrecognized field, proving deserialization discards it rather than
  erroring — every existing test only used a struct literal, never a
  real JSON string with something extra in it.
- **FR-SEC-004**: a pin test on `SecurityResponseAction::SuspendProcess
  .action_type()`. The module's own doc comment already explains this is
  a closed-enum guarantee the compiler enforces structurally, not a
  runtime check — the test adds a little value (catches an accidental
  string change) without overclaiming what it proves.
- **FR-STO-002** (remainder): `latency_ms`'s `Supported` and
  `Unavailable` branches, directly unit-tested — it's a pure private
  function, callable from the same file's `#[cfg(test)]` module.
- **FR-PERF-004**: `score_host`/`score_db` had zero tests beyond the two
  boundaries (100 healthy, 0 fully critical) anywhere in the suite; added
  the actual mixed-tier weighted-penalty arithmetic, plus the
  zero-samples-contributes-no-penalty edge case the function's own doc
  comment already claimed but nothing verified.
- **FR-CAP-001**: new `rust_core/contracts/tests/capability_registry_test.rs`
  — `default_capability_registry()` was never called by any test; the
  two files the generator's grep found only used `Capability` as
  unrelated incidental fixture data, a false trail.
- **FR-PERF-005**: same "no AI call reachable" shape ADR 0068 built for
  TRS §20, just for `ai_ops_core::analysis` instead of
  `ai_ops_core::correlation`. Extended the existing dependency-graph
  script (seeding the BFS from both module prefixes) rather than
  building parallel tooling, and renamed it
  `check-no-ai-reachable-from-analysis-or-correlation.sh` to match.
  Re-verified with the same real-injected-violation methodology ADR
  0068 established (both direct and transitive), now from an
  `ai_ops_core::analysis` seed.
- **FR-DB-006**: new `dbms_lock_contention_test.rs` — two real,
  independent `TestPostgres` sessions, one holding `ACCESS EXCLUSIVE`
  inside an open transaction, the other genuinely blocked trying to
  acquire the same lock; asserts `lock_graph()` reports the real
  blocking PID, not just the blocked one. **A real bug surfaced and
  fixed while building this**: the first attempt had the blocked
  session issue a bare `LOCK TABLE` outside a transaction block, which
  is a real, immediate Postgres error ("LOCK TABLE can only be used in
  transaction blocks") — not a block — so the test passed for the wrong
  reason (an empty edge list) until direct `pg_stat_activity`/`pg_locks`
  inspection caught it.
- **FR-SYS-001**: new `unix_transport_test.rs` — every existing service
  integration test hits the router in-process
  (`tower::ServiceExt::oneshot`); this one actually spawns
  `transport::unix::serve`, waits for the real socket file, asserts its
  permission bits are exactly `0600`, then connects a real
  `tokio::net::UnixStream` and reads a real HTTP response back — the
  first test in this repo to exercise the real transport layer at all.

**2 are left open, deliberately, not gamed**:
- **FR-SYS-003** (sampling back-off under resource pressure): a genuine
  structural gap, not a quick test. `HostTelemetrySampler` hardcodes
  `source: RealProcSource` rather than accepting an injectable
  `&dyn ProcSource` the way `parse_cpu_snapshot` and friends already do
  elsewhere in this codebase — until that refactor happens, there's no
  way to drive a controlled `CpuPressure::Critical` fixture through
  `tick()` to prove the back-off interval switch. Named as a real
  follow-up unit, not attempted here.
- **FR-CORR-003** (correlation output promoted to `contracts`): already
  a deliberate, ADR-0017-documented scope decision — `correlation/mod.rs`'s
  own module doc explains `CorrelationResult` deliberately stays
  `core`-internal for now, the same kind of real, named v2-style
  deferral as `FR-FLEET-*`. No test can honestly prove a promotion that
  hasn't happened; writing one to force `OK` would be dishonest, not a
  fix.

## Verification (real, not simulated)

- Every new/moved test run individually and confirmed passing;
  `dbms_lock_contention_test.rs` and `unix_transport_test.rs` each rerun
  3× to rule out timing flakiness (lock polling, socket-creation
  polling).
- Full workspace: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`, and `cargo test
  --workspace --all-features` all clean.
- `dart format`/`flutter analyze`/`flutter test` all clean on the two
  touched console test files.
- `scripts/check-schema-drift.sh`, the two grep gates, `scripts/check-fr-
  id-traceability.sh`, `scripts/check-no-ai-reachable-from-analysis-or-
  correlation.sh` (re-verified against a real injected violation from
  `ai_ops_core::analysis`, not just `ai_ops_core::correlation`), and
  `scripts/check-console-codegen-drift.sh` all pass locally.
- `docs/traceability-matrix.md` regenerated and committed:
  **58 OK, 2 NEEDS-VERIFICATION, 3 MISSING** (of 63 total FR-IDs) — up
  from 37 OK / 23 NEEDS-VERIFICATION / 3 MISSING at the start of this
  unit.

## Consequences

- The only FR-IDs still short of `OK` are the 2 named above (both with a
  documented reason a future reader can act on or consciously accept)
  and the 3 `FR-FLEET-*` v2 exclusions already allowlisted by unit U61.
- `scripts/check-no-ai-reachable-from-analysis-or-correlation.sh` now
  protects two real boundaries (TRS §20, SRS FR-PERF-005) instead of
  one; any future FR-ID with the identical "no AI reachable from X" shape
  should extend this script's seed set rather than add new tooling.
- FR-SYS-003 is now a concretely-scoped, named follow-up unit (inject
  `&dyn ProcSource` into `HostTelemetrySampler`), not a vague backlog
  item.
