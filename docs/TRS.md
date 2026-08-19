# Technical Requirements Specification — AI Ops Coordinator

Status: draft v0.1, bootstrapped ahead of unit U0.
This document specifies *how* the FRs in docs/SRS.md are met. Units implement
against the sections cited in their own prompt; changing a cited section's
contract requires an ADR, not a silent edit.

## 1. Introduction & Purpose

Technical companion to docs/SRS.md. Where the two disagree, SRS wins for *what*,
this document wins for *how*, and an ADR resolves any conflict that survives that
rule.

## 2. Architecture Overview

Single local core service (`rust_core/service`) fronting all telemetry, analysis,
persistence, policy, and (later) actions, behind one versioned local API. A thin
console client consumes that API and contains no business logic. v2 adds a headless
agent (reusing core's library crates) reporting to a coordinator over mTLS.

## 3. Workspace Layout

Cargo workspace with four crates, established in U0 and not restructured by later
units without an ADR:

- `contracts` — every wire type (serde + schemars), the single source schemas are
  generated from.
- `platform` — the `PlatformAdapter` trait plus `linux/` `windows/` `macos/`
  modules; the only crate permitted `unsafe` and the only location permitted
  `std::process::Command` (confined further to `platform/*/exec.rs`).
- `core` — telemetry parsing, persistence, performance analysis, correlation,
  policy, actions, AI integration, security detection. No direct OS calls; goes
  through `platform`.
- `service` — the binary: HTTP-over-UDS/named-pipe server, wiring, CLI entry point.

## 4. Transport & Wire Contract

- Unix Domain Socket on Linux/macOS under `$XDG_RUNTIME_DIR`, mode 0600; Named Pipe
  on Windows. Loopback TCP is not used for the local API, to avoid exposing it to
  other local users/processes by default.
- Axum served over the UDS/pipe listener. All routes versioned under `/api/v1`.
- Every response uses the envelope `{success, timestamp, request_id, data, error}`.
  `error` is a closed enum variant set, never a bare string, so clients can branch
  on it exhaustively.

## 5. Platform Adapter Trait & Capability Model

`PlatformAdapter` is implemented once per OS. A method not supported on a given
platform returns `CapabilityError::Unsupported(reason)` — it must compile and
return this rather than being `#[cfg]`'d out, so the capability registry (SRS
§14) can describe it uniformly across platforms. The capability enum
(Supported/Limited/Unavailable/PermissionRequired) lives in `contracts` so both
server and generated docs consume the identical type.

## 6. Telemetry Pipeline

`trait ProcSource { fn read(&self, path: &str) -> io::Result<String>; }`. Every
Linux parser is written against `&dyn ProcSource`, never against `std::fs`
directly. Production wiring reads `/proc` and `/sys`; tests inject a fixture-backed
implementation reading from `tests/fixtures/proc/<scenario>/`. This is what makes
telemetry parsing snapshot-testable without a live, variable machine.

## 7. Rate Calculation & Clock Discipline

All rates (CPU, network, disk I/O) are computed from a monotonic clock, never wall
clock. If elapsed time between samples exceeds 2x the configured sample interval,
the pipeline emits `SampleGap` instead of a computed rate. A counter that decreases
between samples is a counter reset (NIC reinit, process restart, etc.) and must
emit `CounterReset`, never a negative number.

## 8. Error Handling Strategy

Errors are typed per-domain (`CapabilityError`, `PolicyError`, `DbError`, ...) and
converted to the closed API error enum at the service boundary only.
`clippy::unwrap_used` and `clippy::expect_used` are denied outside test code at the
workspace level; a boundary that can fail must return `Result`, not panic.

## 9. Concurrency Model (Service)

Tokio async runtime. Telemetry sampling, the DB writer (§12), and the API server
run as independent tasks communicating over bounded channels — a slow consumer
(e.g. a stalled console connection) must not block sampling or persistence.

## 10. Security Baseline

`forbid(unsafe_code)` at the workspace level, overridden only inside `platform/*`
where OS FFI requires it. No SQL string literal outside `core/src/dbms/adapters/*`
(grep-gated in CI, §11). No `std::process::Command` outside `platform/*/exec.rs`
(also grep-gated), and that module accepts only `&'static str` program paths plus
typed argument structs — never an assembled shell string.

## 11. CI/CD Pipeline & Guard Rails

`fmt`, `clippy -D warnings`, `test`, `cargo deny`, `cargo audit`, a schema-drift
check (`emit-schemas` output must match the committed `/schemas/*.json`
byte-for-byte), `cargo check` for `x86_64-pc-windows-msvc` and
`aarch64-apple-darwin` even though those platforms only stub out until U12, and the
two grep gates from §10. All required, all blocking merge.

## 12. Data Persistence Schema

SQLite (rusqlite, bundled) via refinery migrations. Pragmas:
`journal_mode=WAL`, `busy_timeout=5000`, `foreign_keys=ON`, `synchronous=NORMAL`.

Core tables (each carrying the columns noted; full DDL lives in `/migrations`):

- `telemetry_snapshots` — raw samples, 7-day retention.
- `telemetry_rollup_hourly` / `telemetry_rollup_daily` — 90-day / 2-year retention.
- `performance_analysis` — host/DB scores and bottleneck verdicts, each row
  carrying `score_version` (SRS FR-PERF-004) so historical rows remain
  interpretable after a weight-set change.
- `audit_events` — every policy decision, action, and purge event, each row
  carrying `prev_hash` and `row_hash` (§14).
- `actions` — proposed/approved/executed/rolled-back actions, each row carrying
  `target_start_time` for the identity-reverification check (see the U8 executor
  pipeline).
- `security_incidents`, `security_events` — detector output (SRS §20).
- `dbms_*` — one family of tables per monitored DBMS capability area (§26–§34),
  namespaced so a future non-PostgreSQL adapter does not collide.

A single dedicated writer task owns the write connection, fed by an mpsc channel;
readers use a small read-only connection pool. No writer pool — SQLite's single-
writer nature is embraced, not fought. A failed migration never drops or recreates
the database (SRS NFR-REL-002); it blocks repository startup only.

## 13. Retention & Rollup Strategy

Raw snapshots 7 days → hourly rollups 90 days → daily rollups 2 years, enforced by
a scheduled job that also runs `incremental_vacuum` and writes its own audit row.
Hard DB size ceiling 500MB, oldest-first eviction beyond that ceiling regardless of
the age-based tiers above.

## 14. Audit Chain Design

`row_hash = sha256(prev_hash || canonical_serialization(row))`, forming a hash
chain over `audit_events` in insertion order. An `audit verify` subcommand walks
the chain and reports the index of the first broken link, giving a tamper-evident
(not tamper-proof) log suitable for a single-writer local deployment.

## 15. Console Architecture

Layered: screens → providers → repositories → API client. Widgets hold no business
logic and make no direct HTTP calls — only providers/repositories touch the API
client. This mirrors SRS FR-CONSOLE-001 at the code-structure level, not just as a
policy.

## 16. Codegen

Console models are generated from `/schemas/*.json` (themselves emitted from
`contracts`, §3–§4). No hand-written DTOs in the console. CI runs codegen and fails
on drift, so contract and client can never silently diverge.

## 17. PostgreSQL Adapter Architecture

`trait DbmsAdapter` (method set specified in §33) with all PostgreSQL SQL confined
to `core/src/dbms/adapters/postgresql/`. tokio-postgres + deadpool-postgres. Every
pooled connection sets `application_name='ai-ops-coordinator'`,
`statement_timeout=5s`, `idle_in_transaction_session_timeout=10s`, and an explicit
`sslmode`.

## 18. Credential Storage

Connection metadata (host, port, database, username, TLS mode) lives in SQLite;
the password lives only in the OS credential store (Keychain / Credential Manager /
Secret Service) and is fetched at connect time. The connection-metadata type has no
field capable of holding a plaintext password — enforced by the type system, per
SRS NFR-PRIV-002, not by review discipline.

## 19. SQL Privacy Model

Default capture mode is `NORMALIZED_QUERY` + fingerprint in every environment.
Raw SQL capture is an explicit per-connection opt-in, stored separately, and
excluded from anything that can reach an AI prompt (§22–§23) unless that same
opt-in is set. Literal redaction and credential-pattern stripping happen before
persistence, not just before display.

## 20. Correlation Engine Design

Implements SRS §31. Inputs are the host analysis (SRS §11) and DB analysis (SRS
§26–§34) for the same time window; output is `CorrelationResult`, a contract type
carrying ranked hypotheses, each with an evidence list, a confidence value, and a
ruled-out list with reasons. No AI call is reachable from this code path — verified
by a CI dependency-graph check in U15, not just by review.

## 21. Scoring Model

Host and DB scores are each computed by a pure function over evidence, parameterized
by a frozen weight table identified by `score_version`. Introducing a new weight
table adds a new `score_version`; it never mutates an existing one, and history
rows are never re-scored retroactively (SRS FR-PERF-004).

## 22. AI Provider Integration

`trait AiProvider`, with an Ollama implementation defaulting to
`http://127.0.0.1:11434`. Every other subsystem must function with this provider
absent, unreachable, slow, or returning garbage — proven by an integration test
that runs the whole request path with the provider disabled, not asserted in
comments.

## 23. Evidence Bundle & Validator

The evidence bundle passed to the AI provider is `{evidence_key -> value}` plus a
numbered candidate list for anything referenceable (SRS FR-AI-004). Untrusted
strings (process names, SQL fragments, container labels, device names) are placed
inside a single delimited, length-capped block the system prompt labels as data,
not instructions. A validator rejects any response containing a numeric without a
resolvable `evidence_ref` (SRS FR-AI-005) — the entire response is discarded, never
partially used. Keyword-based prompt-injection filtering is deliberately not
implemented; the closed output schema plus candidate-id indirection is the
documented security boundary (see the corresponding ADR from unit U6).

## 24. Policy Engine Typestate Design

`ActionRequest -> Validated<ActionRequest> -> PolicyApproved<Action> -> Executed`,
each stage a distinct type in a distinct module with private inner fields.
`PolicyApproved` is constructible only inside the policy module; the executor's
only public entry point accepts `PolicyApproved`. This makes a policy bypass a
compile error rather than a review question.

## 25. Approval Lifecycle

An approval is single-use and bound to `(action_id, target_identity,
parameters_hash)`, with a default 5-minute expiry. Replay, expiry, parameter
mutation, and target change after approval are all rejected states, each with a
dedicated test (SRS FR-POL-003).

## 26. Action Executor Pipeline

Fixed order: schema validation → policy evaluation → approval check → capability
check → protected-resource check → capture previous state → apply → verify →
audit. Each stage is independently unit-testable and none may be skipped for a
given action type.

## 27. Target Identity & Re-verification

Process targets are identified by `(pid, start_time_ticks)`; DB session targets by
`(backend_pid, backend_start)`. Identity is re-verified immediately before apply
*and* immediately before rollback; a mismatch aborts with `TARGET_CHANGED` rather
than acting on a reused identifier (guards against PID/session reuse races).

## 28. Server Agent Architecture

The v2 agent (`rust_core/agent`) is headless: it reuses the `core`/`platform`
telemetry and DBMS-adapter crates directly and has no dependency on the console.
It runs the same capability model and never assumes network reachability to the
coordinator is continuous.

## 29. Coordinator Architecture

The coordinator (`rust_core/coordinator`) persists fleet inventory, telemetry
rollups, alerts, audit, policies, and RBAC (§46) in PostgreSQL — a deliberate
asymmetry from the agent's local SQLite, since the coordinator is the shared,
durable system of record for the fleet.

## 30. Agent–Coordinator Protocol

mTLS with per-agent identity and certificate rotation. Enrollment: a short-lived
join token is exchanged for a CSR, which the coordinator signs. Expired or revoked
certificates are rejected at the TLS layer, not application-checked after the
fact. Every agent→coordinator request carries a nonce and timestamp; the
coordinator rejects requests outside its replay window or with a reused nonce.

## 31. Store-and-Forward & Backfill

An agent disconnected from the coordinator continues sampling into its local
SQLite buffer. On reconnect it backfills without loss up to a bounded ring size; if
the ring fills before reconnection, the oldest data is dropped and the gap is
recorded explicitly rather than silently closed.

## 32. Fleet Data Model

Coordinator-side schema mirrors the agent's domain tables (telemetry rollups,
performance analysis, audit, actions) plus fleet-specific tables: agent inventory,
enrollment records, certificate state, and per-agent connectivity history.

## 33. DbmsAdapter Trait — Method Set

`trait DbmsAdapter` exposes, at minimum: `version_info`, `list_databases`,
`list_sessions`, `query_stats` (capability-gated on `pg_stat_statements`),
`lock_graph`, `table_stats`, `index_stats`, `replication_status`, `relevant_gucs`,
`temp_file_activity`, `deadlock_history`, `long_transactions`,
`idle_in_transaction_sessions`, plus, from U8 onward, a narrow, explicitly-typed
action surface (e.g. `analyze_table(verified_identifier)`) — never a generic
"execute SQL" method. All are read-only through U4; the action surface is added
only in U8, under policy gating (§24–§27).

## 34. Benchmark Statistical Methodology

**PERF-1** (statistical rigor for benchmarking, binding on unit U9): a baseline is
a window of at least 60 seconds and at least 30 samples; if its coefficient of
variation exceeds a configured stability threshold, the run aborts with
`BASELINE_UNSTABLE` and no change is applied. The post-change window matches the
baseline's duration and sampling exactly. Outcomes compare *distributions*
(bootstrap confidence interval on the median, or a Mann-Whitney U test) — never two
point measurements — and resolve to IMPROVED / REGRESSED / INCONCLUSIVE / UNSTABLE,
with INCONCLUSIVE as the honest default, displayed with equal prominence to a win.
Every result records confounders: process-count delta, thermal state, AC/battery
transitions, other active tuning plans, and sample gaps in either window.

## 35. Rollback Design

Rollback triggers: REGRESSED, application failure, instability, thermal
throttling, user cancel, timeout, or a policy change. Rollback re-verifies target
identity (§27) before acting and is itself an audited action; a failed rollback is
a CRITICAL audit event surfaced in the UI, not merely logged.

## 36. Tuning Engine & Automation Modes

A profile (BALANCED, HIGH_PERFORMANCE, BATTERY_SAVER, DEVELOPMENT, CUSTOM) is a
declarative desired-state description translated into candidate typed actions —
never a script. Automation modes are RECOMMEND_ONLY (default), ASK_BEFORE_CHANGES,
and AUTO_LOW_RISK; the last admits only actions that are REVERSIBLE, LOW risk, and
have a locally recorded history of IMPROVED outcomes. At most one plan may be in
flight per target; a concurrent attempt is rejected, not queued.

## 37. Security Detector Framework

Each detector is a deterministic rule over telemetry (or DBMS signals, SRS §34)
with a stable id, a documented rationale, and thresholds tunable per-rule.
Detectors never depend on the AI provider to fire (SRS FR-SEC-001). Response
actions are limited to a small closed set (e.g. `SUSPEND_PROCESS`,
`BLOCK_DEVICE`), always require approval, and the `ActionType` enum structurally
cannot contain a firewall/AV-disabling variant.

## 38. Windows Platform Notes

windows-rs. Performance counters/WMI for telemetry, Job Objects for process
priority, a Windows Service host for the core, `SetPriorityClass` +
`SetProcessAffinityMask` for the two U8 actions. ETW and the Windows Filtering
Platform are explicitly deferred past v1.x.

## 39. macOS Platform Notes

Public APIs only: libproc, `host_statistics64`, IOKit, and an SMAppService
privileged helper for anything requiring elevation. Endpoint Security and Network
Extension are explicitly out of scope for v1.x (both require Apple-granted
entitlements). TCC-denied permissions report `PermissionRequired` with a pointer to
the relevant System Settings pane rather than failing silently.

## 40. Performance Budgets & Self-Overhead

The core measures its own CPU% and RSS continuously and exposes them on
`/api/v1/health`. Target ceilings: under 2% of one core and under 150MB RSS over a
10-minute idle window on a reference machine. When host CPU pressure is HIGH or
CRITICAL (SRS §6), the core backs its own sampling interval off to 5 seconds. AI
inference (§22) is refused outright — not merely deprioritized — when memory
pressure is HIGH/CRITICAL or thermal state is THROTTLING, since the model is
itself a heavy consumer of the resource it would be diagnosing.

## 41. Alerting & Hysteresis

Fleet-facing alerts (v2) deduplicate, apply hysteresis around thresholds, support
per-rule suppression windows, and respect a maintenance-window concept. A metric
oscillating across a threshold must produce one alert with state transitions, not a
new alert per crossing.

## 42. Replication & Backup Monitoring

Replication state per SRS §32. Backup state is read from pgBackRest/WAL-G/barom
where one of those is present on the monitored instance; if none is detected the
capability reports Unavailable rather than guessing at backup health. Automatic
replica promotion and automatic failover are not implemented, full stop (SRS
FR-FLEET-002).

## 43. Deployment Topologies

v1.x: single core process per machine, console connects locally over UDS/named
pipe. v2: any number of headless agents plus one coordinator; the coordinator is
not required for v1.x functionality to keep working on a single machine.

## 44. Observability of the Product Itself

The product's own audit log, health endpoint, and self-overhead metrics (§40) are
the primary means of observing the product itself — no separate telemetry pipeline
is introduced just to monitor the monitor.

## 45. Testing Strategy & Fixture Discipline

Linux telemetry parsing is tested exclusively against captured `/proc`/`/sys`
fixtures (TRS §6), never against the live machine, so tests are deterministic and
portable. DBMS adapter tests run against real PostgreSQL containers across the
supported version matrix. Every FR-ID in docs/SRS.md must be traceable to at least
one test; unit U15 generates and checks this traceability matrix before every tag.

## 46. RBAC Roles & Permission Matrix

Coordinator-enforced roles (minimum set): VIEWER (read-only fleet/telemetry
access), OPERATOR (VIEWER plus approve/execute actions within policy), ADMIN
(OPERATOR plus policy and RBAC administration). Role checks happen server-side on
every coordinator request; the console/agent is never treated as a security
boundary (SRS FR-FLEET-003). The full role-to-permission matrix is generated
documentation, kept in sync with the coordinator's authorization code by the U15
docs-regeneration gate.
