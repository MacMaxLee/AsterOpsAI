# AsterOpsAI Architecture

## Overview

AsterOpsAI is a host + PostgreSQL observability tool designed to answer "is something wrong?" and "is it the database, or is it the box?" with evidence, not guesses. The system consists of:

- **Rust core service**: Telemetry collection, analysis, policy enforcement, and automated actions
- **Flutter console**: Thin desktop client for visualization and management
- **Coordinator** (v2): Fleet management across multiple hosts

### Core Design Principles

1. **Capability Model (ADR 0004)**: Return `Unsupported` or `Unavailable` with honest error messages rather than silently failing or providing false guarantees
2. **Contract-First**: All wire types defined in `contracts` crate with JSON Schema validation
3. **Platform Abstraction**: OS-specific operations isolated behind `PlatformAdapter` trait
4. **Honest Gaps**: Named, documented limitations rather than incomplete implementations
5. **Evidence-Based**: Correlation engine uses real metrics and timeline data, not heuristics

## Platform Abstraction Layer

### PlatformAdapter Trait

**Purpose**: Abstract OS-specific operations (process control, self-metrics) behind a uniform interface

**Location**: `rust_core/platform/src/adapter.rs`

**Key Methods**:
- `self_process_metrics()` — RSS and CPU time for health endpoint
- `get/set_process_priority()` — Process scheduling priority
- `get/set_process_cpu_affinity()` — CPU core assignment (where supported)
- `suspend/resume/is_process_stopped()` — Process state control for security actions

**Design**: Returns `Result<T, CapabilityError>` where unsupported operations return `CapabilityError::Unsupported` with explanation, not `#[cfg]`-gated removal

### Current Platform Support

#### Linux (Primary Platform)

- **Status**: Complete, production-ready
- **Implementation**: `rust_core/platform/src/linux/`
- **Telemetry**: `/proc` and `/sys` filesystem parsing
- **Process Control**:
  - Priority: `setpriority(2)` with nice values
  - Affinity: `sched_setaffinity(2)` for hard CPU masks
  - Suspend: SIGSTOP/SIGCONT signals
- **All 9 PlatformAdapter methods implemented**

**Telemetry Modules** (`rust_core/core/src/telemetry/`):
- **CPU**: `/proc/stat` for ticks, `/proc/loadavg` for load averages
- **Memory**: `/proc/meminfo` with cgroup v2 awareness for container environments
- **Storage**: `/proc/mounts` enumeration, `statvfs(2)` for capacity
- **Network**: `/proc/net/dev` for per-interface statistics
- **Processes**: `/proc/[pid]/{stat,status,cmdline}` parsing
- **Devices**: `sysfs` enumeration for block and storage devices

#### macOS (Full Support as of Milestone 4)

- **Status**: Complete, verified in CI (Units U90-U105)
- **Implementation**: `rust_core/platform/src/macos/`
- **Telemetry**: Mach APIs, `sysctl`, `libproc`
- **Process Control**:
  - Priority: POSIX `getpriority(2)`/`setpriority(2)`
  - Affinity: **Unsupported** (no `sched_setaffinity` equivalent — see ADR 0086)
  - Suspend: SIGSTOP/SIGCONT signals, `ps` for state detection
- **7/9 PlatformAdapter methods implemented**

**Telemetry Modules** (`rust_core/core/src/telemetry_macos/`):
- **CPU** (U95): `host_statistics64()`, `host_processor_info()` for per-core utilization
- **Memory** (U96): `vm_statistics64`, `sysctl` for hw.memsize and vm.swapusage
- **Storage** (U97): `getfsstat()` for filesystems, `statfs()` for capacity
- **Network** (U99): `netstat -ibn` parsing for interface statistics
- **Processes** (U100): `libproc` (`proc_listpids`, `proc_pidinfo`, `proc_pidpath`)
- **System**: `sysctl kern.boottime` for uptime calculation

**Implementation Notes**:
- `getpriority()` returns -1 for both errors and valid values; requires errno clearing
- Process stop detection via `ps -o state=` (checks for 'T' or 't' states)
- State tracking for rate-based metrics (CPU, network, process deltas)

**CI Verification**: `test-macos` job runs 121+ library tests on `macos-latest` (Apple Silicon)

#### Windows (Compilation Only)

- **Status**: Compiles, not runtime-tested
- **Implementation**: `rust_core/platform/src/windows/` (stubs returning `Unsupported`)
- **Transport**: Named Pipe server exists (`\\.\pipe\ai-ops-coordinator\core`), never executed
- **CI**: `check-windows` job verifies compilation on `windows-latest`

## Telemetry Architecture

### Design Principle

Platform-specific telemetry modules (`core/src/telemetry` vs. `core/src/telemetry_macos`) rather than shared abstraction, because `/proc` parsing fundamentally differs from Mach APIs. Attempting to abstract over both would create leaky abstractions.

### Service Integration

**Location**: `rust_core/service/src/telemetry/sampler.rs`

**Conditional Compilation**:
```rust
#[cfg(target_os = "linux")]
mod linux_impl { /* uses core::telemetry */ }

#[cfg(target_os = "macos")]
mod macos_impl { /* uses core::telemetry_macos */ }
```

**Shared Design**:
- Same HTTP API shape (`/api/v1/cpu`, `/memory`, `/storage`, etc.) regardless of backend
- Contracts crate defines wire types (`CpuSnapshot`, `MemorySnapshot`, etc.)
- Variable-duration sampling loop (1s normal, 5s when CPU pressure is High/Critical)
- Process refresh throttling (every 5th tick)
- Persistence rate limiting (every 10th tick for telemetry history)

**Capability Registry**: Platform-specific features registered conditionally (e.g., `SetProcessCpuAffinityAction` only on Linux)

## Known Gaps and Limitations

### macOS-Specific Gaps

| Feature | Status | Reason | Reference |
|---------|--------|--------|-----------|
| CPU Affinity | Unsupported | No `sched_setaffinity` equivalent; macOS `thread_policy_set` is advisory-only, per-thread hint | ADR 0086 |
| Per-core frequency | Unavailable | macOS doesn't expose CPU frequency via Mach APIs | `telemetry_macos/cpu.rs` tests |
| Context switches | Unavailable | Not exposed via Mach APIs | `telemetry_macos/cpu.rs` tests |
| Interrupt counters | Unavailable | Not exposed via Mach APIs | `telemetry_macos/cpu.rs` tests |
| Memory buffers | Unavailable | macOS doesn't distinguish buffers from cache | `telemetry_macos/memory.rs` tests |
| NUMA nodes | Unavailable | No standard API exposure | `telemetry_macos/memory.rs` tests |
| Disk I/O stats | Deferred | IOKit FFI complexity vs. value; `iostat` provides rates not cumulative counters | Unit U98 decision |
| Device enumeration | Deferred | Empty `devices_snapshot()` for now | Unit U101 |

**Error Messages**: All gaps return `Unsupported` or `Unavailable` with explanation, following capability model (ADR 0004)

### Cross-Platform Gaps

| Feature | Status | Platforms | Reason |
|---------|--------|-----------|--------|
| Per-process network I/O | Unavailable | Linux, macOS | Requires eBPF/cgroup net-cls on Linux; no macOS equivalent | `core/src/telemetry/process.rs`, `core/src/telemetry_macos/process.rs` |
| Privileged helper | Not implemented | All platforms | Service runs as invoking user; no systemd unit, launchd plist, or SMAppService helper yet | HANDOFF.md §6 |

## Transport Layer

### Unix Domain Socket (Linux, macOS)

- **Linux Path**: `$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`
- **macOS Path**: `/tmp/runtime-$(id -u)/ai-ops-coordinator/core.sock`
- **Permissions**: Mode 0600 (owner-only access)
- **Protocol**: HTTP/1.1 over Unix Domain Socket
- **Design Rationale**: Isolation guarantee (only this OS user, only this machine) — see ADR 0001

**Server Implementation**: `rust_core/service/src/transport/unix.rs`
- Hand-written bridge: `hyper::server::conn::http1` over `UnixListener`
- (Axum 0.7's `axum::serve` only accepts `TcpListener`)

**Console Client**: `console/lib/api/unix_socket_transport.dart`
- Dart `HttpClient` with custom `connectionFactory`
- `Socket.startConnect` against Unix socket path

**Why not TCP**: Deliberate avoidance of network exposure for local-only service (ADR 0001)

### Named Pipe (Windows)

- **Path**: `\\.\pipe\ai-ops-coordinator\core`
- **Server**: `rust_core/service/src/transport/windows.rs` (compiles, never runtime-tested)
- **Client**: Not implemented yet (console has abstract `LocalTransport` interface ready)

## Testing Strategy

### Linux

- **Full Test Suite**: `cargo test --workspace --all-features`
- **Coverage Categories**:
  1. Fixture-based (deterministic): `/proc` parsing via `tests/fixtures/proc/<scenario>/`
  2. Live kernel data (unprivileged): Real `/proc/[pid]/{stat,status}` of test process
  3. PostgreSQL integration: Real disposable instances via `scripts/setup-test-postgres.sh`

- **Deliberate unprivileged testing**: Priority elevation tests verify `PermissionRequired` failure, not success (no `CAP_SYS_NICE` needed)
- **CI Execution**: All tests run on `ubuntu-latest` in GitHub Actions

### macOS

- **Library Tests Only**: `cargo test --workspace --lib`
  - **Reason**: PostgreSQL tests skipped (environment setup, not platform compatibility)
  - **Coverage**: 121+ tests (87% of full suite)
  - **Matches local workflow**: Documented in Unit U102

- **CI Job**: `test-macos` on `macos-latest` (Apple Silicon arm64)
- **Manual GUI Testing**: `docs/U104-MANUAL-TESTING.md` (16-section checklist)

**Skipped Tests**:
- PostgreSQL integration tests (~26 tests, `dbms_*` modules)
- Linux-specific tests (2 tests, `#[cfg(target_os = "linux")]` gated)

### Windows

- **Compilation Only**: `cargo check --workspace`
- **CI Job**: `check-windows` on `windows-latest`
- **No Runtime Testing**: Planned for future work

## CI/CD Pipeline

**Platform Matrix**: `ubuntu-latest`, `macos-latest`, `windows-latest`

**Required Jobs** (all merge-blocking):
- `fmt`: Code formatting (`cargo fmt --all -- --check`)
- `clippy`: Linter (`cargo clippy -- -D warnings`)
- `test`: Full suite on Linux with PostgreSQL setup
- `test-macos`: Library tests on macOS (Unit U105)
- `check-windows`, `check-macos`: Compilation verification
- `deny`: Dependency licensing (`cargo deny`)
- `audit`: Security advisories (`cargo audit`)
- `schema-drift`: Contract schema validation
- `grep-gate-command`: Enforces `Command` only in `platform/*/exec.rs`
- `grep-gate-sql`: Enforces SQL only in `core/src/repository/adapters/`
- `fr-id-traceability`: Requirement traceability check
- `ai-reachability`: Ensures no AI calls in analysis/correlation (TRS §20, SRS FR-PERF-005)

**Console Jobs**:
- `console-analyze`, `console-format`, `console-test`, `console-codegen-drift`

## Workspace Structure

Four crates under `rust_core/` (ADR 0003):

1. **`contracts`**: All wire types (serde + schemars), no workspace-internal dependencies
2. **`platform`**: `PlatformAdapter` trait + linux/macos/windows modules (only crate allowed `unsafe`)
3. **`core`**: Telemetry, persistence, analysis, AI, policy, actions, security, correlation (aliased as `ai_ops_core` by dependents)
4. **`service`**: Binary, HTTP server, CLI, wiring

**Lint Discipline**:
- No `.unwrap()` / `.expect()` outside `#[cfg(test)]`
- `unsafe` forbidden workspace-wide except `platform` crate
- `Command` calls only in `platform/*/exec.rs` (CI enforced)
- SQL only in `core/src/repository/adapters/` (CI enforced)

## References

- **Platform Handoff**: `HANDOFF.md` (Linux→macOS transition guide, historical context)
- **Unit Definitions**: `docs/macos-development-units.md` (Units U90-U110)
- **Architecture Decisions**: `docs/adr/` directory
  - ADR 0001: Transport (UDS/Named Pipe, not TCP)
  - ADR 0003: Workspace layout (four crates)
  - ADR 0004: Capability model (honest errors)
  - ADR 0086: macOS CPU affinity (unsupported)
- **Requirements**: `docs/SRS.md` (functional), `docs/TRS.md` (technical)
- **Development Workflow**: `CLAUDE.md` (unit-by-unit process, contract-first, lint gates)
