# AsterOpsAI Source Code Map

A file-by-file reference across the whole codebase: what each file is
for, its key exports, and how it relates to the rest of the system.
Organized by crate/app, then by directory. ~300 source files total
(24 `contracts`, 12 `platform`, 114 `core`, 38 `service`, ~120 console
`.dart` — 72 of which are auto-generated and covered as one group).

## Table of contents

1. [Workspace overview](#workspace-overview)
2. [`rust_core/contracts`](#contracts) — wire types
3. [`rust_core/platform`](#platform) — OS abstraction
4. [`rust_core/core`](#core) — business logic
   - [telemetry/](#core-telemetry)
   - [dbms/](#core-dbms)
   - [security/](#core-security)
   - [policy/](#core-policy)
   - [correlation/](#core-correlation)
   - [analysis/](#core-analysis)
   - [actions/](#core-actions)
   - [ai/](#core-ai)
   - [benchmark/](#core-benchmark)
   - [repository/](#core-repository)
   - [tuning/](#core-tuning)
5. [`rust_core/service`](#service) — the binary
6. [`console`](#console) — Flutter client

---

## Workspace overview

AsterOpsAI is a four-crate Rust workspace plus a Flutter console,
wired together by one strict dependency direction:

```
contracts  ←──────────────┐  (wire types; zero internal deps)
platform   ←───────────┐  │  (OS abstraction; only unsafe/Command-permitted crate)
core       ← depends on both, is the business logic
service    ← depends on all three; the actual binary
console    ← talks to `service` only over a Unix socket / named pipe,
             never links against any Rust crate directly
```

`core`'s package name is literally `core` — every dependent aliases it
(`ai_ops_core = { package = "core", ... }`). Two CI-enforced grep gates
hold real architectural invariants, not style preferences:
`std::process::Command` may only appear inside `platform/*/exec.rs`,
and `unsafe` may only appear inside `platform/`. A third gate
(`cargo-modules`-based) proves no AI-module item is ever reachable from
`core::analysis` or `core::correlation` — the deterministic "what's
wrong" pipeline is provably independent of the optional AI layer.

---

<a id="contracts"></a>
## `rust_core/contracts/src/`

This crate is the single source of truth for every type that crosses
the HTTP API boundary between `service` and the console client. It has
**zero workspace-internal dependencies** by design, so every type here
is a deliberately parallel, field-for-field mirror of the "real"
domain type living in `core` — never a re-export. Every struct/enum
derives both `serde::{Serialize, Deserialize}` and `schemars::JsonSchema`.

### Top level

**`lib.rs`** — Crate root; re-exports the most commonly consumed types
and defines `API_VERSION = "v1"`.

**`bin/emit_schemas.rs`** — The `emit-schemas` binary: calls
`schemars::schema_for!` on every wire type (bare and wrapped in
`Envelope<T>`/`GatedValue<T>`/`HistoryResponse<T>`) and writes each to
`/schemas/*.schema.json`. CI's schema-drift gate reruns this and fails
the build on any diff.

**`ai.rs`** — `RiskLevel`, `MetricClaim`, `Observation`,
`Recommendation`, `AiExplanation` (SRS FR-AI-001..005). Deliberately
parallel to (not reused from) `core::ai::schema`.

**`analysis.rs`** — `HostBottleneck`, `HostDomain`, `Tier`, `Evidence`,
`DomainSignal`, `HostVerdict` (SRS FR-PERF-001, unit U19). `Evidence`
is reused by `correlation.rs`'s `Hypothesis`.

**`capability.rs`** — `Capability`, `GatedValue<T>`,
`CapabilityFamily`, `default_capability_registry()` — the model that
lets any response honestly say "not fabricated, genuinely unsupported"
(SRS FR-CAP-001/002). Foundational; almost everything else depends on it.

**`correlation.rs`** — `RootCause`, `Hypothesis`, `RuledOut`,
`CorrelationResult` (SRS FR-CORR-002, unit U20). Reuses
`analysis::Evidence`.

**`dbms.rs`** — The full PostgreSQL observability wire surface:
`SessionState`, `SessionInfo`, `LockEdge`, `QueryStat`, `TableStat`,
`IndexStat`, `StandbyInfo`, `ReplicationStatus`, `GucValue`,
`TempFileActivity`, `DeadlockInfo`, `LongTransaction`,
`IdleInTransactionSession`. Largest single file in the crate.
`QueryStat` is always wrapped in `GatedValue<Vec<QueryStat>>` since
`pg_stat_statements` may not be installed.

**`envelope.rs`** — `Envelope<T>` (`{success, timestamp, request_id,
data, error}`), the universal response wrapper.

**`error.rs`** — `ApiError` (BadRequest/NotFound/Unsupported/
PermissionRequired/Unavailable/Internal), the closed wire-facing error
taxonomy; also `thiserror::Error` for use as a real Rust error type.

**`health.rs`** — `HealthResponse`, `SelfMetricValue<T>` — a
two-variant sibling of `telemetry::MetricValue<T>` used specifically
for self-monitoring so `/health` degrades gracefully instead of faking
a `0`.

**`policy.rs`** — `PendingActionSummary`, `ActionProposalOutcome`,
`ResumableActionSummary` (units U13, U26, U29). Mirrors
`core::policy::PolicyOutcome`'s three variants.

**`security.rs`** — `SecurityIncidentSummary` (unit U15/U78) —
including `detector_id`/`resource_kind`/`resource_name` so a console
suppress action has a real target.

**`tuning.rs`** — `TuningPlanSummary`, `TuningPlanOutcome`,
`TuningCandidateOutcome` (units U14, U23).

### `telemetry/` subdirectory

**`telemetry/mod.rs`** — aggregator; re-exports all ten sibling files.

**`telemetry/cpu.rs`** — `CpuSnapshot` (utilization, frequency, load
averages, interrupts, pressure).

**`telemetry/device.rs`** — `DeviceSnapshot`, `DeviceInfo`,
`DeviceKind` — attached block/removable storage inventory; `trusted`
tracks in-process device-trust state.

**`telemetry/history.rs`** — `HistoryTier`, `HistoryStat`,
`CpuHistoryPoint`, `MemoryHistoryPoint`, `StorageHistoryPoint`,
`NetworkHistoryPoint`, `HistoryResponse<T>` — time-bucketed rollups at
raw/hourly/daily tiers.

**`telemetry/memory.rs`** — `MemorySnapshot`, `NumaNodeMemory`
(host/cgroup memory, swap, NUMA breakdown).

**`telemetry/metric_value.rs`** — `MetricValue<T>` (Supported/
SampleGap/CounterReset/Unavailable) — the most widely reused type in
the crate.

**`telemetry/network.rs`** — `NetworkSnapshot`, `NetworkInterfaceInfo`
— every numeric field is `MetricValue<f64>`.

**`telemetry/pressure.rs`** — `MemoryPressure`, `CpuPressure` —
deterministic 4-tier classification (ADR 0006 thresholds).

**`telemetry/process.rs`** — `ProcessSnapshot`, `ProcessInfo`,
`ProcessCategory` — the one telemetry type reaching directly for
`capability::Capability` (per-process disk/network I/O gating).

**`telemetry/storage.rs`** — `StorageSnapshot`, `VolumeInfo` — per-
volume capacity, throughput, IOPS, latency.

**`telemetry/system_status.rs`** — `SystemStatusResponse` — the
closest thing to a single dashboard-summary type (uptime, pressure
tiers, sample interval, full capability map).

### How `contracts` works

`emit-schemas` calls `schema_for!` on each type — bare and wrapped in
the generic envelopes — and writes the result to `/schemas/*.schema.json`,
which the console's codegen consumes to generate typed Dart bindings.
CI's schema-drift gate fails the build if the committed schemas differ
from what the crate's types actually produce, so a `contracts` change
is incomplete until schemas are regenerated and committed in the same
change. Two generic wrapper patterns recur everywhere: `Envelope<T>`
as the outer shape for every response, with `error: Option<ApiError>`
as the closed error taxonomy; and `GatedValue<T>`/`Capability`/
`MetricValue<T>`/`SelfMetricValue<T>` as the "maybe genuinely
unsupported" wrappers that let a response honestly report
`Unavailable`/`PermissionRequired`/`SampleGap` instead of ever
fabricating a placeholder value.

---

<a id="platform"></a>
## `rust_core/platform/src/`

The OS-abstraction boundary: one trait (`PlatformAdapter`), one error
type (`CapabilityError`), three per-OS implementations selected at
compile time. The **only** crate in the workspace permitted `unsafe`
code; its `*/exec.rs` modules are the **only** place permitted to call
`std::process::Command` — both enforced by CI grep gates.

**`adapter.rs`** — Defines the whole contract: `PlatformAdapter` trait
(`platform_name`, `arch`, `self_process_metrics`, `get/set_process_
priority`, `get/set_process_cpu_affinity`, `suspend_process`/
`resume_process`/`is_process_stopped`); `ProcessSelfMetrics`;
`ProcessPriority` (Idle..High, deliberately no Realtime tier per ADR
0013); `CpuAffinityMask`.

**`error.rs`** — `CapabilityError` (Unsupported/PermissionRequired/
Unavailable/Io) plus `to_capability()`, the one place this internal
error crosses into the wire type `contracts::Capability`.

**`lib.rs`** — Crate root; `#[cfg(target_os = ...)]`-gates the three OS
modules; `current_platform_adapter() -> Box<dyn PlatformAdapter>`
picks the concrete implementation at compile time.

**`linux/exec.rs`, `macos/exec.rs`, `windows/exec.rs`** — pre-created,
currently-empty placeholders for a future `std::process::Command`-based
action surface, gated by the same CI script regardless of OS.

**`linux/mod.rs`** — **Real, working** `LinuxPlatformAdapter` — every
trait method implemented for real (no `Unsupported` returns anywhere).
Delegates to `process_control` for priority/affinity/suspend-resume;
implements `self_process_metrics` directly via `getrusage(2)`.

**`linux/process_control.rs`** — **Real.** Priority (reads `/proc/
[pid]/stat`'s nice field; writes via `libc::setpriority`), CPU affinity
(reads `/proc/[pid]/status`'s `Cpus_allowed_list`; writes via
`libc::sched_setaffinity`), suspend/resume (`libc::kill` +
SIGSTOP/SIGCONT), `is_stopped` (parses `/proc/[pid]/stat`'s state
field). Getters use `/proc` text parsing (avoids `getpriority(2)`'s
ambiguous `-1`/errno dance); setters use real `unsafe` syscalls.

**`linux/proc_source.rs`** — **Real.** `ProcSource` trait + `RealProcSource`
— the ONLY production path permitted to read `/proc`/`/sys` for
telemetry, designed specifically so `core::telemetry`'s parsers are
fixture-testable instead of live-machine-only.

**`linux/storage.rs`** — **Real.** `VolumeCapacity`, `volume_capacity()`
via `statvfs(2)` — capacity isn't exposed via any `/proc`/`/sys` text
file, so this is a deliberate exception to the `ProcSource` path.

**`macos/mod.rs`** — **100% STUBBED.** `MacosPlatformAdapter` — every
method other than `platform_name`/`arch` returns
`Err(CapabilityError::Unsupported("... not implemented yet, see unit
U12"))`. Comments name the real target APIs: `setpriority(2)` (same as
Linux), `thread_policy_set`/taskpolicy-style CPU affinity tagging (no
direct `sched_setaffinity` equivalent), `kill`+SIGSTOP/SIGCONT for
suspend/resume.

**`windows/mod.rs`** — **100% STUBBED**, identical shape.
`WindowsPlatformAdapter`. Comments name: `SetPriorityClass`/
`GetPriorityClass` (TRS §38's own named API), `SetProcessAffinityMask`/
`GetProcessAffinityMask` (TRS §38's other named API), per-thread
`SuspendThread`/`ResumeThread` (Windows has no process-level SIGSTOP
equivalent — a real API-shape difference, not just naming).

### How `platform` works

`current_platform_adapter()` picks one of three implementations at
compile time. **Today, only Linux is real** — working process
priority, CPU affinity, suspend/resume/is-stopped, self-process
metrics, plus the `ProcSource`/`RealProcSource` abstraction that backs
all of `core::telemetry`'s host-metric sampling, plus a real
`statvfs`-based `volume_capacity`. **macOS and Windows are 100%
stubbed** — no process priority, no CPU affinity, no suspend/resume, no
self-process metrics, and (having no platform-specific telemetry
module at all) no telemetry sampling either. See the Mac/Windows
transition documents for what real implementations would need.

---

<a id="core"></a>
## `rust_core/core/src/`

Package name is literally `core` (aliased `ai_ops_core` by
dependents). Telemetry, persistence, analysis, policy, actions, AI,
security, correlation, benchmark, tuning — all business logic lives
here.

<a id="core-telemetry"></a>
### telemetry/

**`mod.rs`** — Crate-facing entry point for real Linux `/proc`/`/sys`
telemetry; re-exports every parser's types plus the shared cgroup-v2
path helper. Also carries `testing::FixtureProcSource` (test-only
`ProcSource` reading `tests/fixtures/proc/<scenario>`).

**`context.rs`** — `SampleContext { now, elapsed, configured_interval }`
— the caller-supplied "when was this sampled" struct making every
parser deterministic and testable.

**`cpu.rs`** — Parses `/proc/stat`, `/proc/loadavg`, per-core cpufreq,
cgroup v2 CPU quota → `CpuSnapshot`.

**`device.rs`** — Enumerates `/sys/block` devices for later trust
tracking; feeds `security::detector::detect_untrusted_device`.

**`error.rs`** — `TelemetryError::{Io, Parse}` — fatal parse failures
only (soft per-metric unavailability is encoded inline as
`MetricValue::Unavailable`).

**`memory.rs`** — Parses `/proc/meminfo`, cgroup v2 memory/swap limits
(unit U79 — real cgroup-scoped swap accounting, not just host-wide),
NUMA topology → `MemorySnapshot`.

**`network.rs`** — Parses `/proc/net/dev` → per-interface rates, with
counter-reset/sample-gap handling.

**`pressure.rs`** — `classify_cpu_pressure`/`classify_memory_pressure`
— deterministic, documented-threshold 4-tier classification (ADR 0006).

**`process_classify.rs`** — Pure, I/O-free rule-based process
categorization (System/DbmsEngine/UserApplication/BackgroundService/
Unknown).

**`process.rs`** — Parses `/proc/[pid]/{stat,status,cmdline,cgroup,io}`
for every process → `ProcessSnapshot`, including per-process disk I/O
where permitted. `read_start_time_ticks` is shared with
`actions::host::target_verifier`.

**`rate.rs`** — Shared counter-to-rate conversion: detects `SampleGap`
(elapsed > 2× interval) and `CounterReset` before ever computing a
rate — no metric ever fabricates a negative value.

**`storage.rs`** — Parses `/proc/self/mountinfo` + `/proc/diskstats` →
per-volume I/O rates/latency. Capacity/free fields deliberately left
`Unavailable` here — filled by `service` via `platform::linux::storage`.

**`system_status.rs`** — Assembles the top-level `SystemStatusResponse`.

**How telemetry works**: every parser takes `&dyn ProcSource` (never
touches `std::fs` directly) + `SampleContext` and returns a
`MetricValue<T>`-wrapped snapshot, so a caller can never mistake "we
don't know" for a real zero. `rate.rs` centralizes gap/reset
arithmetic across every counter-based parser. Pure classification
(`process_classify.rs`, `pressure.rs`) is deliberately separated from
I/O. `service`'s sampler loop calls these on an interval and persists
via `core::repository`; `analysis::host` later reads that history back
— telemetry itself never classifies "is something wrong."

<a id="core-dbms"></a>
### dbms/

**`mod.rs`** — Every wire-agnostic PostgreSQL domain type plus the
central `DbmsAdapter` trait (13+ read-only methods).

**`adapters/mod.rs`** — Namespace where the SQL-outside-adapters grep
gate permits raw SQL literals.

**`adapters/postgresql/mod.rs`** — The real `PostgresAdapter`,
delegating each trait method to a `queries::*` function. `connect`
calls `credential_store::fetch` then `role_check::validate` before
returning — a constructed adapter has always passed the least-
privilege check.

**`adapters/postgresql/queries.rs`** — Every raw SQL literal: version,
databases, sessions, locks, table/index stats, replication, GUCs, temp
files, deadlocks, long transactions, role/privilege security-detection
inputs, auth-log config. Runs every raw-query-text field through
`privacy::sanitize_query` first.

**`capability.rs`** — `Gated<T>` (Supported/Limited/Unavailable/
PermissionRequired) — a payload-carrying degrade type distinct from
`contracts::Capability`.

**`connection_metadata.rs`** — `ConnectionMetadata` (structurally
incapable of holding a password — NFR-PRIV-002), `TlsMode`
(Disable/Prefer/Require/VerifyCa/VerifyFull — the last two added unit
U77/ADR 0082), `Environment`, `PasswordRef`.

**`credential_store.rs`** — The only place a plaintext password ever
exists transiently in-process; backed by the real OS keyring.

**`log_tail.rs`** — Reads PostgreSQL's own CSV server log to detect
auth failures by SQLSTATE — deliberately **not** a `DbmsAdapter` trait
method (co-located-deployment-only capability). Byte-offset dedup via
`repository::log_tail_offset`.

**`pool.rs`** — Builds the connection pool with mandatory settings
(`application_name`, `statement_timeout=5s`, `idle_in_transaction_
session_timeout=10s`); branches TLS connector construction on
`TlsMode`, including custom rustls verifiers `EncryptOnlyVerifier`
(Require: encrypt but don't verify identity) and `ChainOnlyVerifier`
(VerifyCa: real chain validation, deliberately hostname-tolerant).

**`privacy.rs`** — NFR-PRIV-001: redacts credential-shaped substrings
unconditionally, normalizes literals unless `capture_raw_sql` is
explicitly opted in.

**`role_check.rs`** — Refuses a superuser connection in Production
unless explicitly overridden; builds the override audit event.

**`version_matrix.rs`** — Minimum supported PostgreSQL version (13.0)
+ per-capability version gates.

**How dbms works**: `DbmsAdapter` is the single abstraction point;
`PostgresAdapter` is its only real implementation. Credentials never
live in `ConnectionMetadata` and are fetched only at connect time from
the OS keyring. Every raw signal destined for the security detectors
(role grants, GUC changes, auth-log config, client addresses) is
exposed as a plain query result here and turned into a `DetectedEvent`
one layer up in `security::detector`. `dbms` itself never classifies
health — `analysis::db` does, from a `DbEvidenceBundle` the caller
assembles from one round of adapter calls.

<a id="core-security"></a>
### security/

**`mod.rs`** — Documents the four FR-SEC hard boundaries (pure
detectors, mandatory correlation, security-flag override, closed
response-action enum).

**`detector.rs`** — Pure, deterministic detector rules, no I/O, no AI:
`detect_superuser_override`, `detect_untrusted_device`,
`detect_guc_change`, `detect_role_superuser_granted`,
`detect_unusual_client_address`, `detect_role_membership_granted`,
`detect_table_privilege_granted`, `detect_auth_failure` (with real
repetition-based severity escalation, unit U75/ADR 0080).

**`error.rs`** — `SecurityError::{Repository, Action, Serde}`.

**`incident.rs`** — `record_event` — the typed entry point over the
repository's atomic find-or-open-incident logic (FR-SEC-002).

**`response.rs`** (Linux-only) — The one real closed-enum security
response action: SIGSTOP/SIGCONT process suspension, wired into
policy/actions as always-approval-required, high-risk.

**`severity.rs`** — `Severity` (Info..Critical) — SRS pins only the
two endpoints; the three middle levels are a documented judgment call.

**`suppression.rs`** — FR-SEC-005: records a suppression rule plus a
tamper-evident audit event.

**How security works**: detector functions are pure — they take
already-fetched evidence and return `Option<DetectedEvent>`.
`incident::record_event` is the boundary where a `DetectedEvent`
becomes durable: checks suppression, then correlates into an existing
open incident or opens a new one, atomically. The one sanctioned
automated response is a normal high-risk `ActionKind` — security never
bypasses the policy engine.

<a id="core-policy"></a>
### policy/

**`mod.rs`** — Documents the typestate hard boundary (`ActionRequest
-> Validated -> PolicyApproved` is compile-time-enforced, unbypassable)
and "no AI in the decision path."

**`approval.rs`** — Stage 3: `grant`, `reject` (the veto path),
`authorize` (single-use, checked against exact target/parameters/
resource, constructs the real `ActionKind`).

**`error.rs`** — `PolicyError` + mapping functions preserving specific
lifecycle errors rather than flattening them.

**`evaluate.rs`** — Stage 2: computes the risk+environment decision,
persists the lifecycle row, audits every outcome including denials.
`PolicyOutcome::{AutoAllowed, PendingApproval, Denied}`.

**`hash.rs`** — SHA-256 of an action's parameters JSON for approval-
binding (detects parameter mutation between proposal and execution).

**`registry.rs`** — `ActionTypeRegistry` — the exhaustive action-type-
to-behavior map (risk level, reversibility, validator, constructor).

**`request.rs`** — `ActionRequest` — the raw, untyped starting point.

**`resource.rs`** — FR-POL-006's `ProtectedResourceRegistry` — a list
of protected processes/services/devices consulted once, at
execute-time.

**`risk.rs`** — Pure `(RiskLevel, Environment) -> PolicyDecision`;
Production is never more permissive than Development for the same
risk level.

**`status.rs`** — `ActionStatus` state-machine enum matching the
`actions` table's `status` column.

**`target.rs`** — `TargetIdentity::{Process, DbSession}` — pid+start-
time-ticks or backend_pid+start.

**`typestate.rs`** — `Validated<T>`/`PolicyApproved` with private,
module-scoped constructors — a bypass is a compile error.

**`validate.rs`** — Stage 1: schema validation against the registry.

**How policy works**: `validate` → `evaluate` → `approval::{grant,
authorize}` (with `reject` as veto), each stage enforced by the
typestate chain so no code outside `core::policy` can fabricate an
approved action. Pure decision logic, zero AI, zero direct OS
interaction — the actual side-effecting `ActionKind` implementations
live in `core::actions`/`security::response`.

<a id="core-correlation"></a>
### correlation/

**`mod.rs`** — Documents FR-CORR-001's pure, deterministic combination
of `HostVerdict` + `DbHealthVerdict` — no I/O, no AI.

**`cause.rs`** — `RootCause` — the fixed 9-value enum (FR-CORR-002's
exact minimum list).

**`confidence.rs`** — Two distinct confidence formulas: DB evidence
averages check severities (single-poll snapshot); host evidence uses
sustained-crossed-fraction.

**`correlate.rs`** — The real logic: maps DB/host categories onto 8
root causes, derives the 9th (ClientSideApplication) arithmetically
from the other eight's max confidence via `m*(1-m)`, ranks/rules-out
by threshold.

**`hypothesis.rs`** — `Hypothesis`, `RuledOut`, `CorrelationResult`.

**How correlation works**: takes an already-computed `HostVerdict` +
`DbHealthVerdict` (never raw telemetry), computes confidence per cause,
ranks (>0.1) or rules out with a human-readable reason. Pure
arithmetic, zero I/O, zero AI — the same boundary `analysis` and
`security::detector` establish for themselves, CI-enforced for this
module specifically.

<a id="core-analysis"></a>
### analysis/

**`mod.rs`** — Asserts the FR-PERF-005 hard boundary: nothing here may
call, import, or be influenced by an AI model.

**`db.rs`** — Classifies DB health across 11 categories (Availability,
ConnectionSaturation, Latency, SlowQueries, LockWaits, Deadlocks,
RollbackRatio, TempFileUsage, LongTransactions, ReplicationLag,
BloatProxies) from one poll's `DbEvidenceBundle`.

**`evidence.rs`** — `Evidence { metric, observed, threshold, unit,
window_start, window_end }` — shared by every classification.

**`host.rs`** — Classifies host bottleneck (the "differentiator") over
a chronological window of persisted telemetry, distinguishing
sustained pressure from background-service noise.
`HostBottleneck`/`HostDomain`/`HostVerdict`.

**`score.rs`** — Two frozen, versioned weight tables (`HOST_SCORE_
VERSION`, `DB_SCORE_VERSION`) converting classified tiers into a
0-100 score; a new weight set gets a new version, never mutates in
place.

**`thresholds.rs`** — All documented judgment-call 4-tier thresholds
(~11 `classify_*_tier` functions) plus sustained-signal window
constants.

**How analysis works**: `host::classify_host` (window of persisted
rows, requires sustained High/Critical fraction) and `db::classify_db`
(one live snapshot, 11 independent categories) are the two pure
classifiers, each producing a versioned 0-100 `score` with per-
category `Evidence`. Neither performs I/O nor has any AI dependency —
by CI-enforced design, no code path from `analysis` or `correlation`
can reach `core::ai`.

<a id="core-actions"></a>
### actions/

**`mod.rs`** — Module root for the action executor + real Linux host
actions.

**`context.rs`** — `ActionContext { platform: Arc<dyn PlatformAdapter> }`.

**`error.rs`** — `ActionError` covering every failure mode of the
nine-stage pipeline.

**`executor.rs`** — `execute()` — the sole entry point from
`PolicyApproved` through stages 4-9 (capability check, protected-
resource check, security-flag check, capture previous state,
re-verify + apply, verify, audit).

**`kind.rs`** — `ActionKind` trait every real mutating action must
implement.

**`rollback.rs`** — `rollback`/`rollback_by_row_id` — a rollback is a
*new* lifecycle row (never mutates the original), re-verifying target
identity first.

**`target_verifier.rs`** — `TargetVerifier` trait, re-verified
immediately before apply and before rollback.

**`host/mod.rs`** — Registers the two real U8 action types (Linux-
only).

**`host/priority.rs`** — `host.set_process_priority` — real
`setpriority(2)` via `PlatformAdapter`. Risk Low, NOT reversible (ADR
0013 privilege asymmetry).

**`host/affinity.rs`** — `host.set_process_cpu_affinity` — real
`sched_setaffinity(2)`. Risk Low, reversible.

**`host/target_verifier.rs`** — `ProcessTargetVerifier` — re-reads
`/proc/[pid]/stat`'s start-time-ticks field to detect PID reuse.

**How actions works**: the executor's `execute()` is the only path to
`apply()` — re-checks capability/protected-resource/security-flag
status, captures pre-change state, re-verifies identity immediately
before mutating, applies, verifies, audits immutably. `rollback()`
mirrors this discipline and always inserts a new row. Two real
`ActionKind`s ship (priority, affinity); this same pipeline is reused
unmodified by `benchmark::run` and `tuning::plan`.

<a id="core-ai"></a>
### ai/

**`mod.rs`** — Three hard boundaries: evidence-only input, citation-
based output validation, graceful degradation via `try_explain`.

**`bundle.rs`** — Pure functions turning `HostVerdict`/
`DbHealthVerdict`/`CorrelationResult` into the numbered, referenceable
`EvidenceBundle` shape the provider is allowed to see — `EvidenceItem`,
`Candidate`, `build_host_bundle`, `build_db_bundle`,
`build_correlation_bundle`.

**`prompt.rs`** — Builds system/user prompts, placing all untrusted
data inside one delimited, length-capped `=== DATA ===` block.

**`provider.rs`** — `AiProvider` trait + `try_explain` — the mandatory
degrade-gracefully entry point.

**`ollama.rs`** — The one real `AiProvider`: a bare `hyper` HTTP
client (no TLS, no pool) talking to Ollama's `/api/generate`.

**`schema.rs`** — `RawAiExplanation` (deserialized straight from
provider JSON) vs. `AiExplanation` (only `validator::validate` can
construct).

**`validator.rs`** — The real security boundary (not keyword-based
filtering): every numeric claim and candidate reference must resolve
into the sent `EvidenceBundle`, or the whole response is discarded.

**How ai works**: deterministic output is flattened into a numbered
`EvidenceBundle` — the only thing the model ever sees. The prompt
isolates untrusted text in one delimited block. The closed-schema
validator enforces a citation model as the actual defense against
prompt injection, not keyword filtering. `try_explain` never
propagates or panics — AI is strictly additive.

<a id="core-benchmark"></a>
### benchmark/

**`mod.rs`** — Hard boundaries: no verdict from a single point-in-time
sample; no dependency on `core::ai`/`core::analysis` scoring.

**`config.rs`** — `BenchmarkConfig` — TRS §34 numeric minimums plus two
documented judgment calls (stability CV threshold, significance level).

**`confounders.rs`** — `Confounders` — TRS §34's five named
confounders; three have no real data source anywhere and stay
honestly `None`.

**`error.rs`** — `BenchmarkError` aggregating sampling/policy/
action/repository/serde failures.

**`sampler.rs`** — `MetricSampler` trait + `HostCpuUtilizationSampler`
(real `/proc/stat` reader, Linux-only), decoupled entirely from the
SQLite telemetry pipeline.

**`stats.rs`** — Hand-rolled, dependency-free statistics (ADR 0014):
coefficient of variation, median, Mann-Whitney U with tie correction
and a normal-approximation p-value (own `erf`).

**`verdict.rs`** — `resolve()` → Improved/Regressed/Inconclusive/
Unstable, plus the distinct pre-flight `BASELINE_UNSTABLE` abort.

**`run.rs`** — The orchestrator: baseline window → abort-if-unstable →
drive the real unmodified action pipeline → post-change window →
resolve verdict → rollback on Regressed/Unstable (TRS §35).

**How benchmark works**: wraps the real, unmodified action pipeline in
a measure-before/measure-after statistical harness — baseline (≥60s/30
samples, aborts if unstable) → validate/evaluate/authorize/execute →
post-change window → Mann-Whitney comparison → one of four verdicts,
Inconclusive the honest default. Regressed/Unstable trigger automatic
rollback.

<a id="core-repository"></a>
### repository/

The SQLite persistence layer. Migrations live in the workspace-root
`/migrations/` (19 `.sql` files), embedded via
`refinery::embed_migrations!`.

**Architecture note (ADR 0007)**: deliberately no writer connection
pool. `writer.rs` spawns exactly **one** dedicated OS thread owning the
single write `Connection` for the process lifetime, fed by a bounded
`tokio::mpsc` channel of `WriteCommand`s. Because there is exactly one
writer thread, every check-then-write sequence (policy's compare-and-
swap, security's find-or-open-incident, tuning's uniqueness check, the
audit chain's `next_id` state) is race-free with **no additional
locking**. Reads go through a completely separate `r2d2` pool of 4
read-only connections, safe concurrently with the writer under WAL
mode.

**`mod.rs`** — `RepositoryHandle`, `init()`, one async wrapper function
per operation.

**`writer.rs`** — `WriteCommand` enum (every mutation) + `spawn()`.

**`reader.rs`** — `checkout()` — one pooled read-only connection.

**`connection.rs`** — Opens the write connection (pragmas,
`auto_vacuum=INCREMENTAL` for new files) and the read pool
(`journal_mode=WAL`, `busy_timeout=5000`).

**`error.rs`** — `RepositoryError` with dedicated variants for real
expected conditions (`TuningPlanAlreadyInFlight`,
`ConflictingActionInFlight`, etc.) — not a catch-all.

**`models.rs`** — Every row/insert-payload struct; deliberately *not*
the live typed contracts, keeping this layer decoupled. Includes
`IncidentDetail` (unit U78) shared by list/close incident paths.

**`audit.rs`** — The tamper-evident hash chain: `row_hash =
sha256(prev_hash || canonical_encoding(row))`, hand-rolled encoding.
`verify_chain` walks the whole chain.

**`benchmark.rs`** — `benchmark_runs` CRUD; atomic `mark_rolled_back`
guards against double-rollback.

**`client_address_history.rs`, `device_trust.rs`, `guc_history.rs`,
`role_history.rs`** — Four near-identical "have we seen X before"
seen-tracking tables backing DBSEC detectors (V16/V12/V14/V15).

**`history.rs`** — Tier selection + read queries backing console
history endpoints; `query_recent_snapshots` feeds
`analysis::classify_host`.

**`log_tail_offset.rs`** — Durable byte-offset bookkeeping for the
auth-failure log tailer.

**`migrations.rs`** — Runs pending Refinery migrations at startup;
idempotent, never drops/recreates on failure.

**`performance_analysis.rs`** — Insert-only persistence for periodic
derived health verdicts.

**`policy.rs`** — `actions` table CRUD; `transition`'s atomic
compare-and-swap is the one function every lifecycle step goes
through.

**`retention.rs`** — Rollup computation, age-purge, size-ceiling
(500MB) eviction — runs on the writer's own connection.

**`role_membership_history.rs`, `table_privilege_history.rs`** — Two
seen-tracking tables with an added `reconcile()` (unit U74) that
forgets stale pairs so a revoke-then-regrant fires again instead of
being silently suppressed forever.

**`security.rs`** — `security_events`/`security_incidents`/
`security_suppressions` — find-or-open-incident correlation, severity
escalation (never downgrades, unit U75), suppression check, incident
closing (unit U76). `resource_is_flagged` is called synchronously by
the actions executor's security-flag stage.

**`telemetry_store.rs`** — Insert-only raw `telemetry_snapshots`
persistence.

**`time.rs`** (private) — The one timestamp format used everywhere:
millisecond-precision, `Z`-suffixed RFC3339.

**`tuning.rs`** — `tuning_plans` CRUD; maps the partial-`UNIQUE`-index
violation to `TuningPlanAlreadyInFlight`.

**How repository works**: all mutation flows through exactly one
dedicated OS thread — the idiomatic way to combine rusqlite's
synchronous API with async callers, and what makes every compare-and-
swap race-free without extra locking. Reads never touch the writer
thread. On top sit a hash-chained audit log, tiered telemetry storage
with retention, the atomic action-lifecycle table, and a family of
"have we seen X" tables backing the DB security detectors, several
with explicit revocation-reconciliation logic.

<a id="core-tuning"></a>
### tuning/

**`mod.rs`** — Two hard boundaries: at most one plan in flight per
target (real DB partial `UNIQUE` index), `AutoLowRisk`'s strict
three-part gate.

**`mode.rs`** — `AutomationMode { RecommendOnly, AskBeforeChanges,
AutoLowRisk }`.

**`profile.rs`** — Translates `TuningProfile` (Balanced/
HighPerformance/BatterySaver/Development/Custom) into a concrete
`DesiredState` via a small, explicit, hand-picked table (ADR 0015).

**`candidates.rs`** — Diffs live process state against `DesiredState`,
proposing an `ActionRequest` only for fields that actually differ.

**`history.rs`** — FR-TUNE-003's "locally recorded improved-outcome
history" check.

**`error.rs`** — `TuningError`, mapping the real partial-`UNIQUE`
violation to `PlanAlreadyInFlight`.

**`plan.rs`** — `start_plan` — the orchestrator: reserve slot → diff →
build candidates → route each through `AutomationMode`-gated
evaluation (and, for `AutoLowRisk`, full auto-execution) → mark plan
COMPLETED/REJECTED.

**How tuning works**: `RecommendOnly` never touches policy at all
(pure suggestion). `AskBeforeChanges` routes through the real
`policy::evaluate`, landing in the same shared, generic `PENDING_
APPROVAL` inbox every other action type uses (no tuning-specific
console affordance was ever needed). `AutoLowRisk` additionally
requires the action type be `reversible` and have a locally recorded
improved-outcome benchmark history before auto-executing through the
unmodified `core::actions::execute` pipeline.

---

<a id="service"></a>
## `rust_core/service/src/`

The binary: HTTP-over-Unix-socket/named-pipe server, wiring, CLI.

### main.rs & lib.rs

**`main.rs`** — Entry point. `serve()` builds the platform adapter,
spawns `self_metrics`, initializes the repository, spawns `retention`,
spawns the telemetry sampler, resolves/connects the DBMS adapter,
conditionally spawns `dbms_security_sweep`, builds the action registry,
resolves policy/AI config, assembles `AppState`, builds the router,
serves it. Also handles a one-off `audit verify` CLI subcommand.

**`lib.rs`** — Library root exposing all modules so integration tests
can exercise the router without a real socket.

### config & wiring

**`config.rs`** — Resolves the default SQLite DB path
(`$XDG_DATA_HOME`/`%APPDATA%`), refusing to guess on unset env vars.

**`dbms_config.rs`** — Resolves an optional Postgres connection config
purely from `ASTEROPS_DB_*` env vars — all-or-nothing.

**`ai_config.rs`** — Resolves an optional `AiProviderConfig` from
`ASTEROPS_AI_*` env vars.

**`policy_config.rs`** — Resolves the deployment `Environment`
(Development/Staging/Production) from `ASTEROPS_POLICY_ENVIRONMENT`.

**`dbms_connect.rs`** — Builds the pool, runs least-privilege
`role_check::validate`, wraps in `PostgresAdapter`.

**`state.rs`** — `AppState` — the single shared struct injected into
every handler.

### background schedulers

**`retention.rs`** — Runs the retention sweep every 15 minutes.

**`self_metrics.rs`** — Samples this process's own CPU%/RSS every 5s.

**`dbms_security_sweep.rs`** — Runs the five DBMS security/config
detectors on an independent 30-second schedule (unit U72/ADR 0078),
decoupled from HTTP polling.

**`telemetry/sampler.rs`** — Host telemetry sampler; ticks at 1s
(backing off to 5s under CPU pressure), refreshing process/device
enumeration every 5th tick, persisting every 10th.

**`telemetry/persist.rs`** — Maps live `HostTelemetrySnapshot` into
the `TelemetrySnapshotRow` shape `core::repository` persists.

### transport

**`transport/unix.rs`** — **Real.** Serves over a Unix Domain Socket
at `$XDG_RUNTIME_DIR/.../core.sock`, mode 0600 — deliberately not TCP
(ADR 0001).

**`transport/windows.rs`** — **Real implementation, only compile-
checked.** Serves over a Windows Named Pipe
(`\\.\pipe\ai-ops-coordinator\core`) using the same hyper/
`TowerToHyperService` bridge as `unix.rs`. CI's `check-windows` job
type-checks this on a real Windows runner but never executes it.

### api/v1/*.rs handlers

**`mod.rs`** — Assembles the full `/api/v1/*` route table.

**`health.rs`** — `GET /health`.

**`cpu.rs`, `memory.rs`, `storage.rs`, `network.rs`, `processes.rs`,
`devices.rs`, `system_status.rs`** — Seven thin handlers, each reading
one field off the shared telemetry snapshot.

**`history.rs`** — `GET /history/{cpu,memory,storage,network}`.

**`actions.rs`** — `POST /actions/propose`, `GET /actions/resumable`.

**`policy.rs`** — The approval inbox: `pending`/`grant`/`reject`/
`rollback`.

**`tuning.rs`** — `plans`/`start`.

**`security.rs`** — `incidents`/`close` (unit U76)/`suppress`.

**`dbms.rs`** — 11 endpoints, one per `DbmsAdapter` poll method, each
degrading to 503 with no DB configured.

**`analysis.rs`** — `host`, `host/explain`, `db/explain`,
`correlation`, `correlation/explain`.

### actions.rs (the registry)

**`actions.rs`** — `build_action_registry()` (registers the real
`ActionKind`s), `target_verifier()`, and the three `*_error_to_api`
mapping functions shared across `policy.rs`/`tuning.rs`/`actions.rs`
handlers.

### middleware.rs, response.rs

**`middleware/request_id.rs`** — Assigns a per-request UUID, echoed as
`x-request-id`.

**`response.rs`** — Wraps a handler's `Result<T, ApiError>` into
`Envelope<T>` and picks the HTTP status from the closed `ApiError`
enum.

### Overall request lifecycle

`main.rs::serve()` builds every long-lived dependency into one
`AppState`; `api::router(state)` nests `api/v1::router()` under
`/api/v1`, layering `assign_request_id` middleware. The transport
layer accepts raw socket/pipe connections and feeds them through
hyper's HTTP/1 handler into that same tower `Router`. Each request gets
a UUID stamped before route matching dispatches to a handler, which
pulls what it needs from `State<AppState>`, does its work (often via
`ai_ops_core`/`platform` calls), and returns an `ApiResult<T>` that
`response.rs` serializes into a versioned `Envelope<T>`.

---

<a id="console"></a>
## `console/lib/` (Flutter)

~120 `.dart` files; 72 under `generated/models/` are auto-generated
1:1 from `contracts`' JSON schemas and covered as one group below.

### main.dart & app shell

**`main.dart`** — Boots Flutter bindings, loads `SharedPreferences`,
boots the Riverpod `ProviderScope`.

**`screens/root_gate.dart`** — The single hard gate for API-version
incompatibility, checked once before anything else mounts.

**`screens/app_shell.dart`** — Persistent frame: a `NavigationRail` (14
destinations) beside the active screen, `ConnectionBanner` always
visible above content.

### api/ (transport + client)

**`api/local_transport.dart`** — Abstract `LocalTransport` interface
(never TCP, per ADR 0001).

**`api/unix_socket_transport.dart`** — Concrete implementation: HTTP
over a Unix domain socket via `dart:io`'s `HttpClient` with a custom
`connectionFactory`.

**`api/socket_path.dart`** — Resolves the fixed core socket path,
mirroring the Rust service's own resolution. Unset
`XDG_RUNTIME_DIR` is a hard failure, never a silent fallback.

**`api/api_client.dart`** — The single seam through which every screen
reaches the core (TRS §15) — dozens of typed methods wrapping every
REST endpoint.

**`api/envelope.dart`** — Hand-written mirror of `contracts::
Envelope<T>`.

**`api/api_result.dart`** — `ApiResult<T>` sealed class (`ApiOk`/`ApiErr`).

**`api/api_failure.dart`** — Closed set of every way a call can fail
(SRS FR-CONSOLE-002) — Timeout/Unavailable/MalformedPayload/
ServerError.

**`api/api_version.dart`** — `kSupportedApiVersion = 'v1'`, compared
exactly against `/health`.

**`api/history_query.dart`** — Builds history-endpoint query strings
with explicit wire values (avoiding a previously-hit serde
`rename_all` bug).

### providers/ (Riverpod state)

**`transport_provider.dart`** — One shared `UnixSocketTransport`/
`ApiClient` pair for the app's lifetime.

**`settings_provider.dart`** — Refresh interval + locale, persisted via
`SharedPreferences`.

**`connection_status.dart`** — Polls `/health` every 2s independent of
the refresh setting; derives a sealed connection status
(connecting/connected/reconnecting/unavailable/timeout/malformed/
version-mismatch), guarded against a manual retry racing the timer.

**`telemetry_providers.dart`, `dbms_providers.dart`,
`policy_providers.dart`, `security_providers.dart`,
`tuning_providers.dart`, `analysis_providers.dart`,
`correlation_providers.dart`** — One `StreamProvider.autoDispose` per
data family, each backed by its own `PolledRepository` at the
user's configured refresh interval. Mutations (grant/reject, suppress/
close, etc.) are one-shot `ApiClient` calls issued directly by
screens, not exposed here.

### repositories/

**`polled_repository.dart`** — `PolledRepository<T>` — the one
reusable engine behind every "poll and stream results" provider: owns
a `Timer.periodic`, republishes on a broadcast stream.

### screens/ (one per view)

**`dashboard_screen.dart`** — Landing overview.

**`cpu_screen.dart`, `memory_screen.dart`, `storage_screen.dart`,
`network_screen.dart`, `devices_screen.dart`** — Per-family detail
views.

**`processes_screen.dart`** — Process list plus the most mutation-
heavy dialogs: start a tuning plan, suspend a process (real SIGSTOP),
resume/rollback.

**`policy_inbox_screen.dart`** — Approval inbox; grant/reject/run-now,
always a real server round-trip + re-fetch, never optimistic local
removal.

**`tuning_history_screen.dart`** — Read-only tuning plan history.

**`security_incidents_screen.dart`** — Open incidents with per-row
close and suppress (unit U78's pre-filled, locked dialog) plus a
standalone suppress-any-detector dialog.

**`host_analysis_screen.dart`, `correlation_screen.dart`** — Verdict
views with on-demand ("Explain with AI") lazy-fetched explanation
sections.

**`database_screen.dart`** — The largest screen — 11 independently-
polled PostgreSQL zones plus on-demand DB AI explanation.

**`settings_screen.dart`** — Refresh interval, language, read-only
"About" (connected core version).

**`upgrade_required_screen.dart`** — Full-screen block shown by
`RootGate` on API version mismatch.

### widgets/ (shared)

**`async_result_view.dart`** — Shared loading/failure/data branching
for `AsyncValue<ApiResult<T>>`.

**`connection_banner.dart`** — Persistent "why isn't this fresh"
banner.

**`metric_display.dart`** — The one place server-computed
capability/gated states become a UI treatment (SRS FR-CAP-001) — a
vocabulary map, never new client-side classification.

**`metric_row.dart`, `pressure_badge.dart`, `formatters.dart`** —
Shared layout/formatting; `pressure_badge.dart` renders the server's
already-computed tier, never re-derives it.

### l10n/

Standard Flutter-generated localization scaffolding from ARB source
(English + Traditional Chinese) — every string accessed via
`AppLocalizations.of(context)!`, never inline literals.

### generated/models/

72 files (+ a `models.dart` barrel), produced 1:1 from `contracts`'
JSON schemas by `tool/generate_models.dart`, each headed `// GENERATED
CODE - DO NOT EDIT BY HAND`. Three recurring shapes: plain data classes
with `fromJson`/`toJson`; simple wire enums round-tripped by exact
server string (never derived structurally); and tagged-union "gated"/
"metric value" sealed classes whose `fromJson` dispatches on a `state`
discriminator, forcing exhaustive pattern-matching.

### Overall console architecture

Screens are thin: each watches one or more `StreamProvider<ApiResult
<T>>`s and hands the payload to `AsyncResultView`. Every polled
provider follows one template — watch `apiClientProvider` +
`refreshIntervalProvider`, wrap an `ApiClient` method in a
`PolledRepository<T>`, `autoDispose` tears the timer down once nothing
watches it. Mutations and expensive on-demand AI calls are deliberately
kept out of this polling machinery — issued as one-shot `ApiClient`
calls via `ref.read`, then either invalidating the relevant provider or
showing a real result dialog. `ApiClient` never talks real network TCP
— everything routes through a Unix domain socket (Windows named-pipe
support reserved behind the same `LocalTransport` interface).
