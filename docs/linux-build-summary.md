# AsterOpsAI — Linux Build Summary & Future Improvements

## What AsterOpsAI is

A host + PostgreSQL observability tool that answers two questions an
operator actually has — "is something wrong?" and "is it the database,
or is it the box?" — with real evidence, not guesses. It's built as a
Rust core service (`rust_core/`, four crates) plus a Flutter console
client (`console/`), talking over a Unix domain socket, never real
network TCP. v1 is scoped to a single host + single PostgreSQL
instance; a v2 fleet coordinator is named in the spec but deliberately
not built yet (see "What's deliberately not built" below).

## What's been built, end to end

Development proceeded as ~85 numbered "units," each documented by its
own ADR (`docs/adr/0001` through `0085`). Grouped by what they added:

**Foundations (units ~U0–U3, ADRs 0001–0008).** The four-crate
workspace layout (`contracts`/`platform`/`core`/`service`), Unix-socket
transport (never TCP), contract-first wire types with CI-enforced
schema drift checking, the `PlatformAdapter` OS-abstraction boundary,
SQLite persistence via a single dedicated writer thread (no writer
pool — ADR 0007), and the Flutter console's own architecture.

**Host telemetry (ADRs 0006, 0013).** Real `/proc`/`/sys`-backed
CPU/memory/storage/network/process/device sampling, deterministic
4-tier pressure classification, background-vs-foreground process
attribution, and the two real host-control actions (process priority,
CPU affinity) plus suspend/resume — all real syscalls on Linux, all
independently unit-tested against fixtures.

**PostgreSQL observability (ADRs 0009, 0036–0047, 0053, 0056–0058).**
A `DbmsAdapter` trait with a real `PostgresAdapter` covering sessions,
locks, query/table/index stats, replication, GUCs, temp-file activity,
deadlocks, long transactions, idle-in-transaction sessions — all via
real queries against real, disposable PostgreSQL 13/15/17 test
instances, no mocking. TLS support spans `Disable`/`Prefer`/`Require`/
`VerifyCa`/`VerifyFull` (the last two added late, ADR 0082). A
least-privilege role check refuses unattended superuser connections in
Production.

**Deterministic analysis &amp; correlation (ADRs 0010, 0017, 0024–0025).**
Pure, AI-free host-bottleneck and DB-health classifiers (11 DB
categories, ~10 host bottleneck types), combined into a 9-cause
cross-layer correlation engine — CI-enforced to never be reachable from
an AI call (`check-no-ai-reachable-from-analysis-or-correlation.sh`),
so "what's wrong" is always independently reproducible without an LLM.

**AI explanation layer (ADRs 0011, 0049–0052, 0076).** An optional,
strictly-additive natural-language explanation of an already-computed
verdict (host, DB, or correlation) — schema-validated against the
verdict it's explaining, gated behind a configured Ollama provider,
never itself a source of truth.

**Policy-gated action execution (ADRs 0012, 0018, 0027–0034, 0048,
0055, 0059).** A compile-time-enforced typestate pipeline (`validate`
→ `evaluate` → `approve`/`reject` → `authorize` → `execute`), a real
approval inbox, single-use parameter-hash-bound approvals, rollback/
resume for reversible actions, and duplicate-proposal guarding.

**Security detection (ADRs 0016, 0060–0065, 0079–0081, 0083).** Eight
deterministic detectors (superuser override, untrusted device, GUC
change, role-superuser grant, unusual client address, role-membership
grant, table-privilege grant, authentication failure with real
repetition-based severity escalation), correlated into incidents,
suppressible per-rule/per-resource, closeable (manual, ADR 0081), and
a real per-incident suppress action in the console (ADR 0083).

**Tuning engine (ADRs 0015, 0019, 0022, 0028–0029).** Three automation
modes (RecommendOnly/AskBeforeChanges/AutoLowRisk), the last gated on
reversibility + locally-recorded improved-outcome history, feeding a
real before/after statistical benchmark comparison (Mann-Whitney U,
ADR 0014) to verify whether a change actually helped.

**Console (ADRs 0008, 0021–0023, 0029, 0032, 0034, 0037, 0039, 0041,
0043, 0045, 0047, 0050, 0052).** Every domain above has a real Flutter
screen: dashboard, per-family telemetry, database (11 independently-
polled sections), policy inbox, security incidents, tuning history,
host/correlation analysis with on-demand AI explanation. All screens
are thin — no client-side reclassification of anything the server
already computed (FR-CONSOLE-001), enforced by convention and widget
tests.

**Reliability &amp; process hygiene (ADRs 0066–0075, 0078, 0084–0085).**
An FR-ID traceability matrix (every SRS requirement mapped to a real
implementation + test, CI-checked for drift), a demo-scenario harness
with three real end-to-end correlation proofs, a dedicated background
scheduler for security detection (decoupled from HTTP polling), stale
doc-comment corrections, cgroup-v2-aware swap accounting, and a fixed
flaky test (root-caused by direct reproduction under CPU contention,
not guessed at).

## What's deliberately not built (named, not silently skipped)

- **v2 fleet coordinator.** Scoped in a design-only proposal (ADR
  0071); the three `FR-FLEET-00x` requirements are the only items in
  `docs/traceability-matrix.md` still `NEEDS-VERIFICATION` — by design,
  not oversight.
- **macOS and Windows platform backends.** See the separate Mac/
  Windows transition documents — `platform::{macos,windows}` are
  100%-stubbed `PlatformAdapter` implementations; Windows' HTTP
  transport (named pipe) is real but only compile-checked, never
  run; macOS has no transport implementation at all yet.
- **`ResourceDescriptor` exact-name matching only** — no glob/pattern
  matching for protected-resource checks (deliberately deferred as
  security-sensitive; ADR-flagged, not attempted).
- **Automatic incident resolution.** Incidents close manually only
  (ADR 0081); per-detector "signal cleared" auto-resolution was
  explicitly named and explicitly declined as real, separate,
  larger-scoped work.
- **Multi-DBMS support beyond PostgreSQL, automatic failover, GPU/NPU
  telemetry, AI-originated action execution** — all explicitly out of
  scope for v1 per `docs/SRS.md` §2.

## Recommended future improvements (not yet started, no ADR)

1. **`README.md` is badly stale** — it still says "Status: bootstrapping
   — unit U1 of 16," dating from the very first unit. A five-minute fix
   with real value: whoever opens this repo next reads a description of
   a project barely begun, not the ~85-unit system that actually exists.
2. **Real hardware verification.** Everything to date has run in a
   sandboxed dev container. A dedicated Ubuntu server (already
   planned) is the natural next real-environment check — see the Linux
   Test Plan's §7 for specifics worth re-confirming on bare metal.
3. **`ResourceDescriptor` glob/pattern matching**, if a real use case
   ever needs it (e.g. "protect every process matching `postgres*`")
   — currently exact-name-only, by design, but named as a future widen
   point in its own doc comment.
4. **Automatic incident resolution**, if manual-only closing proves too
   noisy in practice — the harder, larger design question ADR 0081
   explicitly left open (auto-resolve vs. stay manual).
5. **Broaden the demo scenario harness.** Three scenarios exist
   (lock-storm, pool-exhaustion, storage-latency); a fourth
   network-latency or replication-lag scenario would extend the
   strongest end-to-end proof this repo has to the causes it doesn't
   yet demonstrate live.
6. **macOS and Windows** — see the dedicated transition documents.
