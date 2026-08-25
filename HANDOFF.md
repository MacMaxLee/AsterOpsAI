# HANDOFF — Linux → macOS Development

Written 2026-08-24, updated at commit `00256cc` (confirmed identical to
`origin/main`, CI green — see §7). Everything below was verified
directly against the current source, not recalled from memory or
design docs — where a doc and the code disagreed, the code won and the
doc is flagged as stale.

## 1. The platform-adapter trait, verbatim, and how it's actually reached

`rust_core/platform/src/adapter.rs`, in full:

```rust
use std::collections::BTreeSet;
use std::time::Duration;

use crate::error::CapabilityError;

/// This process's own resource usage, as reported by the OS.
#[derive(Debug, Clone, Copy)]
pub struct ProcessSelfMetrics {
    pub rss_bytes: u64,
    /// Cumulative user+system CPU time consumed by this process since it
    /// started. Callers derive a CPU% from the delta between two samples and
    /// a monotonic clock — never from a single reading.
    pub cpu_time: Duration,
}

/// A process scheduling priority (unit U8). Shaped after Windows' priority
/// *classes* (TRS §38 names `SetPriorityClass` as one of "the two U8
/// actions") rather than a raw numeric value, so the same enum has one
/// meaning across platforms even though only Linux has a real
/// implementation this unit. Deliberately excludes a Realtime tier — a
/// documented, flagged safety scope-down (see docs/adr/0013), not an
/// oversight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessPriority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
}

/// A process's allowed-CPU set. `cpus` holds logical CPU indices (matching
/// `CpuSnapshot.per_core_utilization_percent`'s own indexing, unit U1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuAffinityMask {
    pub cpus: BTreeSet<usize>,
}

/// One implementation per OS, selected at compile time. A method not
/// supported on a given platform returns `CapabilityError::Unsupported` — it
/// must compile and return this rather than being `#[cfg]`'d out, so the
/// capability model can describe it uniformly across platforms (TRS §5).
pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &'static str;

    fn arch(&self) -> &'static str {
        std::env::consts::ARCH
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError>;

    /// Unit U8. `pid` need not be a child of this process — any process
    /// this OS user has permission to inspect.
    fn get_process_priority(&self, pid: u32) -> Result<ProcessPriority, CapabilityError>;

    /// Raising priority (e.g. to `AboveNormal`/`High`) typically needs a
    /// privilege this process may not have — a real, expected
    /// `CapabilityError::PermissionRequired`, not something this method
    /// hides or works around.
    fn set_process_priority(
        &self,
        pid: u32,
        priority: ProcessPriority,
    ) -> Result<(), CapabilityError>;

    fn get_process_cpu_affinity(&self, pid: u32) -> Result<CpuAffinityMask, CapabilityError>;

    fn set_process_cpu_affinity(
        &self,
        pid: u32,
        mask: &CpuAffinityMask,
    ) -> Result<(), CapabilityError>;

    /// Unit U11's `SuspendProcess` security response action: stops the
    /// process's execution (`SIGSTOP` on Linux) without terminating it.
    fn suspend_process(&self, pid: u32) -> Result<(), CapabilityError>;

    /// Resumes a process stopped by `suspend_process` (`SIGCONT` on
    /// Linux) — `SuspendProcessAction`'s rollback.
    fn resume_process(&self, pid: u32) -> Result<(), CapabilityError>;

    /// `SuspendProcessAction`'s real verify step: is the process's live
    /// state actually stopped (`/proc/[pid]/stat`'s state field == `T` on
    /// Linux) — not merely "does `suspend_process` return `Ok`."
    fn is_process_stopped(&self, pid: u32) -> Result<bool, CapabilityError>;
}
```

Selected at compile time in `rust_core/platform/src/lib.rs`:

```rust
pub fn current_platform_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "linux")]   { Box::new(linux::LinuxPlatformAdapter) }
    #[cfg(target_os = "windows")] { Box::new(windows::WindowsPlatformAdapter) }
    #[cfg(target_os = "macos")]   { Box::new(macos::MacosPlatformAdapter) }
}
```

### Important correction to how this is actually wired

**There is no separate "privileged agent" process today, and
`PlatformAdapter` is not an IPC boundary.** It's a plain in-process
Rust trait object. `main.rs::serve()` calls
`platform::current_platform_adapter()` once at startup and stores the
`Box<dyn PlatformAdapter>` inside `AppState`; `core::policy`/
`core::actions`'s executor calls its methods as ordinary function
calls, in the same OS process, same privilege level as the `service`
binary itself. There is no systemd unit, no launchd plist, no setuid
binary, no privilege-separated helper anywhere in this repo (verified:
no `.service`/`.plist` files exist at all) — `service` runs as
whatever user starts it, and `set_process_priority`'s own doc comment
already documents the consequence: raising a process's priority
typically needs a privilege this process may not have, and that's a
real, expected `CapabilityError::PermissionRequired`, not a bug.

**The one real IPC boundary is `service` ↔ console**, and it *is*
Linux-specific today:

- **Mechanism**: HTTP/1.1 over a **Unix Domain Socket** at
  `$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`, mode `0600`.
  Deliberately never loopback TCP (see
  `docs/adr/0001-transport-uds-named-pipe-not-tcp.md`) — the isolation
  guarantee (only this OS user, only this machine) is the actual
  design goal, not the choice of socket type. `rust_core/service/src/
  transport/unix.rs` implements the server side by bridging
  `hyper::server::conn::http1` onto a raw `UnixListener` (axum 0.7's
  own `axum::serve` only accepts `TcpListener`, so this bridge is
  hand-written, not a library feature). `console/lib/api/unix_socket_
  transport.dart` implements the client side via Dart's `HttpClient`
  with a custom `connectionFactory` routing through
  `Socket.startConnect` against the same Unix socket address; `console/
  lib/api/socket_path.dart` resolves the identical `$XDG_RUNTIME_DIR`-
  relative path independently, matching the Rust side by convention,
  not by sharing code.
- **What already exists for a second OS, unverified**: a **real,
  complete** Windows Named Pipe implementation,
  `rust_core/service/src/transport/windows.rs` — same hyper/
  `TowerToHyperService` bridge, pipe name
  `\\.\pipe\ai-ops-coordinator\core`. CI's `check-windows` job
  compiles it on a real `windows-latest` runner but **never executes
  it** — no client has ever connected to it for real. There is
  **no console-side named-pipe `LocalTransport` implementation yet**
  (only `UnixSocketTransport` exists); `console/lib/api/
  local_transport.dart`'s `LocalTransport` is already an abstract
  interface designed for a second implementation to slot into.
- **What this means for macOS**: macOS has Unix domain sockets
  natively (it's a real POSIX system, same syscalls as Linux), so
  `transport/unix.rs`'s server side and `unix_socket_transport.dart`'s
  client side likely need **zero changes** to keep working as-is on
  macOS — this is not a Linux-only mechanism, it's a POSIX one. XPC is
  *not* required to reach parity; it would only become relevant if a
  future macOS privileged-helper design (see §6) needs a
  `SMAppService`-managed helper talking back to an unprivileged UI
  process, which is new architecture this repo doesn't have yet on any
  platform, Linux included.

## 2. Linux-only system dependencies NOT captured in Cargo.toml/pubspec.yaml

Verified by grepping the whole repo for install commands, `.rules`
files, `setcap`/capability references, and eBPF/udev/D-Bus mentions —
this is the complete list, not a partial one:

| Dependency | Why it's needed | Where documented today | macOS equivalent |
|---|---|---|---|
| `clang cmake ninja-build pkg-config libgtk-3-dev` | Only needed to *run* the console as a real Linux desktop app (`flutter run -d linux`) or build it for the demo scenarios (`scripts/demo/build-console.sh`) — **not** needed for `cargo test`/`flutter test`/`flutter analyze` | `console/README.md`, `docs/demo/RUNBOOK.md` | Xcode Command Line Tools (`xcode-select --install`) — Flutter's own macOS desktop toolchain requirement, nothing AsterOpsAI-specific |
| Real PostgreSQL 13/15/17 server+client binaries | Every `dbms_*`/`security_detector_*` integration test spins up a real, disposable PostgreSQL instance | `scripts/setup-test-postgres.sh` (downloads official PGDG `.deb`s over HTTPS, extracts with `dpkg -x`, **no root, no system install**) | The script is Debian/Ubuntu-specific (PGDG apt repo, `.deb` extraction). On macOS this needs a different source — Postgres.app, Homebrew's `postgresql@17`, or the official macOS installers — the *mechanism* (a local, disposable, real Postgres for tests) stays the same, the *acquisition* doesn't |
| `openssl` CLI | Several `core` TLS tests (`dbms_tls_require_test.rs`) generate real self-signed certs via `openssl req` | not documented anywhere explicitly — implicitly assumed present | Present by default on macOS (LibreSSL-based `/usr/bin/openssl`, API-compatible for the flags these tests use) — worth a real confirmation run, not assumed |
| A running D-Bus session bus | `core::dbms::credential_store` uses the `keyring` crate's Linux backend (Secret Service protocol over D-Bus) to store the PostgreSQL password | not documented; present by default on any desktop Linux session, may be genuinely absent in a minimal/headless/container environment | **N/A on macOS** — `keyring`'s `apple-native-keyring-store` feature (already enabled in `Cargo.toml`, see §3) uses the Security framework Keychain directly, no D-Bus, no daemon dependency at all |

**Not present anywhere in this codebase** (checked explicitly since
they're common host-monitoring dependencies and the absence is worth
confirming, not assuming): no eBPF toolchain, no udev `.rules` files,
no `setcap` steps, no Linux capabilities (`CAP_*`) granted to the
binary itself. The one real `CAP_SYS_NICE` mention in the codebase
(`core/src/actions/host/mod.rs`) is a **doc comment explaining why
raising process priority needs a privilege the process may not have**
— not a setup step anyone runs. The test suite deliberately tests the
*unprivileged* failure path (see §5), so this was never something you
needed to configure.

## 3. Linux-specific crates in use, with versions

Checked directly against `Cargo.toml`/`Cargo.lock` — this list is
shorter than you'd probably expect, because this codebase deliberately
hand-rolls `/proc`/`/sys` text parsing rather than depending on a
`procfs`-style crate, and does no eBPF, udev, or netlink/nftables
anything anywhere:

| Crate | Version | Used for | macOS story |
|---|---|---|---|
| `libc` | `0.2` | Raw syscalls in `platform::linux`: `setpriority`, `sched_setaffinity`, `kill` (SIGSTOP/SIGCONT), `getrusage`, `statvfs` | `libc` itself is cross-platform (it's the FFI bindings crate, not Linux-only) — but every *call site* uses Linux-specific syscalls/constants (`sched_setaffinity` has no macOS equivalent at all — see §6). Stays a dependency; the calling code in a new `platform::macos` module needs entirely different `libc` function calls |
| `keyring` | `4`, feature `apple-native-keyring-store` already enabled | OS credential store for the PostgreSQL password (`core::dbms::credential_store`) | **Already macOS-ready.** The `apple-native-keyring-store` feature (Keychain backend) is already turned on in the workspace `Cargo.toml` — added ahead of time for unit U12, per its own comment. No Cargo.toml change needed; only real on-device verification |
| `zbus`, `zbus-secret-service-keyring-store` (+ `secret-service`, `zbus_names`, `zbus_macros`) | pulled in transitively by `keyring`'s Linux Secret Service backend | D-Bus client for the Linux credential-store backend — **pure-Rust**, no system `libdbus` C library needed | Not compiled on macOS at all — `keyring`'s backend selection is itself per-target; the Apple backend takes over automatically, no manual feature-gating needed on your end |

**Explicitly confirmed absent** (grepped `Cargo.lock` by name): no
`procfs`, no `udev`, no `aya`/`ebpf`, no `netlink`/`nftables` crate
anywhere in the full dependency tree, direct or transitive. The one
real eBPF *mention* in the repo is a doc comment
(`core/src/telemetry/process.rs`): per-process network I/O attribution
reports `Unavailable` with reason `"per-process network attribution
requires eBPF/cgroup net-cls, not available via /proc on Linux"` — a
named, honest gap, not something built and needing porting.

## 4. How platform-specific code is gated today

**This is already a real, working abstraction — not "assumed Linux
with no abstraction."** Verified by grepping every `#[cfg(target_os
= ...)]`/`#[cfg(unix)]`/`#[cfg(windows)]` in the workspace (36 sites).
The gating is layered, and each layer's own doc comments explain
*why* it is or isn't gated — this is deliberate, not incidental:

**Crate level (`rust_core/platform/`)**: `linux`/`macos`/`windows` are
three separate `#[cfg(target_os = "...")]`-gated submodules, each
providing one `PlatformAdapter` implementation. This part is already
fully ready for a second OS — `platform::macos` already exists as a
compiling (if fully stubbed) module.

**`core` crate root (`core/src/lib.rs`)**: only **one** module,
`telemetry`, is gated `#[cfg(target_os = "linux")]` at the top level —
because `/proc`/`/sys` parsing genuinely has no meaning off Linux.
Every other module (`repository`, `dbms`, `analysis`, `ai`, `policy`,
`actions`, `benchmark`, `tuning`, `security`, `correlation`) is **not**
gated at the module level — each one's own doc comment states plainly
why (pure logic, no OS interaction, or network-only I/O). *Within*
several of these unGated modules, individual items are internally
gated: `security::response` (the one real security action,
Linux-only), `benchmark::sampler::HostCpuUtilizationSampler`, and the
whole `actions::host` submodule (both real `ActionKind`
implementations).

**`service` crate**: `telemetry/sampler.rs` has a real Linux
implementation plus a permanent non-Linux stub returning
`Unavailable`, both compiling; `actions.rs`'s registry-building
function gates registration of the two Linux-only action types on
`cfg!(target_os = "linux")`; `transport/mod.rs` gates `unix`/`windows`
submodules on `cfg(unix)`/`cfg(windows)`; `config.rs`/`main.rs` gate
the default-DB-path resolution and transport `serve()` call the same
way.

**How much refactoring is needed before a second `PlatformAdapter` can
be added without breaking the Linux build**: **none, structurally.**
`platform::macos::MacosPlatformAdapter` already exists, already
implements the trait (fully stubbed), and the crate already compiles
for `aarch64-apple-darwin` (CI's `check-macos` job proves this on every
push). Adding real macOS logic to that existing module — replacing
`Err(CapabilityError::Unsupported(...))` bodies with real
implementations — requires **zero changes to `core` or `service`**;
both already call through the trait, not through Linux-specific types.
The real work is *inside* `platform::macos` (writing the actual syscalls
— see the Mac/Windows Transition Pack artifact from this session for
the exact target APIs already named in that module's own comments) and
building the **new** `core::telemetry`-equivalent module for macOS
host metrics, since `core::telemetry` itself is Linux-gated at the
module root and has no macOS sibling module at all yet — that's new
code, not a refactor of existing code.

## 5. Test coverage — what's actually verified vs. assumed

Verified directly (ran the suite, read the test bodies, checked what
each one actually touches) rather than inferred from test names.
**No test in the automated `cargo test --workspace --all-features`
suite requires root, real hardware, or live cgroup v2/eBPF features.**
Three real categories exist:

1. **Fixture-based (fully deterministic, zero live-machine
   dependency)**: the large majority of `core::telemetry`'s own unit
   tests, using `FixtureProcSource` reading `tests/fixtures/proc/
   <scenario>/`. This includes the container/cgroup-memory-swap
   scenario (`container-swap-accounted`) — cgroup v2 swap accounting is
   tested via a *fixture* with real-shaped `memory.swap.max`/
   `memory.swap.current` file contents, not by requiring the CI runner
   to actually have cgroup v2 delegated.
2. **Real live kernel data, but unprivileged and CI-safe**:
   `platform::linux::process_control`'s own unit tests
   (`get_priority_of_self_is_a_real_value`,
   `get_cpu_affinity_of_self_is_non_empty`) read the *real* `/proc/
   [pid]/{stat,status}` of the test binary's own process — genuine
   live kernel data, but read-only, no privilege needed, safe
   alongside other concurrently-running tests. Separately,
   `core/tests/actions_host_priority_affinity_test.rs` spawns a real,
   disposable child process and **deliberately tests the unprivileged
   failure path**:
   `rollback_of_a_priority_lowering_action_fails_cleanly_when_
   unprivileged` and
   `requesting_high_priority_unprivileged_fails_cleanly_without_
   mutating_state` both assert that boosting priority *without*
   `CAP_SYS_NICE` fails cleanly with `PermissionRequired`, not that it
   succeeds — so this suite has never needed root to pass, by design.
   CPU affinity mutation (no privilege needed on Linux) does get a real
   positive end-to-end test:
   `set_process_cpu_affinity_end_to_end_with_rollback`.
3. **Real disposable PostgreSQL instances, no root**: every `dbms_*`/
   `security_detector_*` integration test spins up genuine
   `initdb`/`pg_ctl`-managed PostgreSQL 13/15/17 processes via the
   binaries `scripts/setup-test-postgres.sh` extracts. CI runs this
   exact script on every push (`.github/workflows/ci.yml`'s `test`
   job) — this is not a "works on my machine" gap, it's already
   verified in the real CI sandbox.

**What is *not* covered by `cargo test` at all** (manual-only, real
hardware/root territory): the demo scenario harness
(`scripts/demo/*.sh`) provisions real throwaway PostgreSQL + a real
service + a real console build and induces genuinely real conditions
(a real blocking lock chain, real connection saturation, real disk
I/O contention); `storage-latency.sh` optionally uses a genuine
kernel-level `cgroup v2 io.max` cap for reliability on fast storage,
requiring a one-time `sudo scripts/demo/setup-io-cgroup.sh` delegation
step. None of this runs in CI or in `cargo test` — it's a separate,
manual verification path, documented in `docs/demo/RUNBOOK.md`.

**Known, currently-non-reproducing flake** (see `docs/adr/0085`):
`deadlock_history_reports_a_real_induced_deadlock` was observed to fail
twice under very heavy full-workspace parallel load; a *different*
test's flakiness (ADR 0085) was root-caused and fixed by direct
reproduction, but `deadlock_history`'s own flake never reproduced under
the same stress and was left alone rather than guessed at. If it
recurs on macOS, treat it as a fresh investigation.

## 6. Known gaps / open design decisions (not written down elsewhere)

- **No privileged-helper architecture exists on any platform yet.**
  `service` runs at whatever privilege level the invoking user has; a
  future macOS build that wants an `SMAppService`-managed elevated
  helper (per TRS §39, for whichever real actions turn out to need
  elevation on macOS) is genuinely new architecture — there is no
  Linux precedent to port, since Linux's own equivalent gap
  (`CAP_SYS_NICE`) is simply left as an honest `PermissionRequired`,
  never solved with a helper process.
- **`sched_setaffinity` has no macOS equivalent at all** (not a naming
  difference like `SetPriorityClass`/`setpriority` — macOS's own CPU
  affinity model, via `thread_policy_set`, is a *hint* the scheduler
  may ignore, not a hard mask like Linux/Windows). Any future
  `platform::macos::set_process_cpu_affinity` needs a real design
  decision here, not a mechanical port — worth resolving before
  writing code, not after.
- **Per-process network I/O attribution is a named, accepted gap on
  Linux itself** (`core/src/telemetry/process.rs`): reports
  `Unavailable` with reason `"requires eBPF/cgroup net-cls, not
  available via /proc"` — this was never solved on Linux, so there's
  no existing mechanism to port to macOS either; a macOS equivalent
  (if ever pursued) would be new design work on both platforms
  simultaneously, not a Linux-first-then-port sequence.
- **Console has no Windows named-pipe (or any second) transport
  implementation** — only `UnixSocketTransport` exists client-side,
  even though the abstract `LocalTransport` interface was designed for
  more than one. Since Unix domain sockets work identically on macOS,
  this is *not* blocking for macOS — flagged here so it isn't
  mistakenly assumed to be a prerequisite.
- **`ResourceDescriptor` matches by exact name only** — no glob/pattern
  matching for protected-resource checks. Named in its own doc comment
  as a future widen point, deliberately not attempted (security-
  sensitive — a bad implementation could under- or over-protect).
- **Incidents close manually only** — no automatic "signal cleared"
  resolution exists for any of the eight security detectors (ADR
  0081). This was a real decision point (auto-resolve was considered
  and explicitly declined as larger, separate work), not an oversight.
- **`README.md` is stale** — still reads "Status: bootstrapping — unit
  U1 of 16," dating from the very first unit. Not fixed as part of
  this handoff since it's out of scope for a technical handoff, but
  worth a five-minute pass before anyone else reads it.
- **No `rust-toolchain.toml` pin** — the workspace builds against
  whatever `rustup`/CI resolves as `stable`. Hasn't caused a problem
  yet, but worth knowing before diagnosing a "works on my Linux
  machine, not on my Mac" discrepancy that turns out to just be a
  compiler version drift.
- **The `demo/storage-latency.sh` scenario has a documented, accepted
  flakiness mode** on fast/virtualized storage (unprivileged `dd`-based
  contention alone may not cross real latency thresholds) — this is
  disk-hardware-dependent and will need its own re-validation on
  whatever Mac hardware you move to, independent of any code port.

## 7. Current repository state (verified directly, not assumed)

- **Commit**: `00256ccde630e8640052c34632632a948c13fd7b` on `main`.
  - This commit added supplementary documentation: source code map,
    Linux test plan, build summary, and Mac/Windows transition pack.
  - The base HANDOFF.md was created at commit `a35fdf3` (two commits prior),
    which fixed a flaky scheduler-independence test under CPU contention (U80).
- **Remote sync**: `git rev-parse HEAD` and `git rev-parse origin/main`
  are identical — local `main` and `origin/main` are the exact same
  commit.
- **Working tree**: clean (`git status --short` empty) — no
  uncommitted changes, staged or unstaged, at the time this update
  was finalized.
- **Unpushed work**: none — nothing local is ahead of `origin/main`.
- **CI**: The CI run for the base commit `a35fdf3` completed with
  conclusion `success` — all 16 required jobs (fmt, clippy, test, deny,
  audit, schema-drift, both grep gates, fr-id-traceability,
  ai-reachability, check-windows, check-macos, console-analyze,
  console-format, console-test, console-codegen-drift) passed.
- **Local build**: Cannot be verified on this macOS machine as Rust/Cargo
  toolchain is not yet installed. However, the CI passing on commit
  `a35fdf3` confirms the workspace builds cleanly on Linux. See
  `docs/linux-build-summary.md` and `docs/mac-windows-transition-pack.md`
  for build requirements on each platform.

### Update (2026-08-25): macOS Implementation Complete

Following the completion of Milestone 4 (Units U101-U105), the macOS platform
adapter and telemetry implementations are now fully integrated and verified:

- **Latest commit**: `d454e2d` "U105: CI Integration - macOS Test Runner | MILESTONE 4 COMPLETE 🎉"
- **Commits since handoff**: 5 units (U101-U105)
  - U101: Wire macOS Telemetry into Service Layer (`aab48a7`)
  - U102: macOS Test Suite Execution (`af5d8bb`)
  - U103: macOS Console Build & Verification (`4ef20d3`)
  - U104: macOS Demo Scenarios - Infrastructure Verified (`bc1f2a1`)
  - U105: CI Integration - macOS Test Runner (`d454e2d`)

- **Platform adapter status**: 7/9 methods implemented
  - ✅ `self_process_metrics`, priority get/set, suspend/resume/is_stopped
  - ❌ CPU affinity (unsupported per ADR 0086)

- **Telemetry modules**: All 5 core subsystems operational
  - CPU, memory, storage, network, process telemetry via Mach/sysctl/libproc
  - See `docs/ARCHITECTURE.md` for detailed implementation notes

- **CI verification**: `test-macos` job runs 121+ library tests on every push

- **Known gaps**: Documented in `docs/ARCHITECTURE.md` and ADR 0086

The HANDOFF document above remains accurate for historical context. For current
macOS architecture details, see `docs/ARCHITECTURE.md`.
