# AsterOpsAI Mac/Windows Transition Pack

Everything to carry into macOS and Windows development: what's
already reusable as-is, what needs real per-OS work (with the exact
APIs already named in the codebase's own comments), a prompt/todo list
per OS, and a current feature list + wishlist. Grounded in what's
actually in the repo today (verified by reading `platform/src/{macos,
windows}/mod.rs`, `service/src/transport/windows.rs`, and
`docs/TRS.md` §38-39) — not speculation.

## 1. What's reusable as-is (no OS-specific work needed)

- **All four Rust crates' business logic** — `contracts`, `core`'s
  `policy`/`actions`/`security`/`analysis`/`correlation`/`ai`/
  `benchmark`/`tuning`/`repository` modules have zero OS-specific code.
  They compile and pass their full test suite on any target already
  (CI's `check-windows`/`check-macos` jobs prove the whole workspace
  type-checks on both).
- **SQLite persistence** (`core::repository`) — `rusqlite`'s `bundled`
  feature compiles SQLite from C source per-target; no OS-specific
  logic in the repository layer itself.
- **The Flutter console's entire architecture** — Riverpod providers,
  `PolledRepository`, all screens/widgets, `l10n` — none of it is
  Linux-specific. Flutter itself supports macOS and Windows desktop
  targets natively.
- **The DBMS layer** (`core::dbms`) — `tokio_postgres`/
  `deadpool_postgres`/`rustls` are all pure-Rust and cross-platform;
  nothing here touches a Linux-only API.
- **The OS credential store abstraction** (`core::dbms::
  credential_store`) — already built on the `keyring` crate, which
  itself supports Secret Service (Linux), Keychain (macOS), and
  Credential Manager (Windows) — likely needs no code change, only
  real on-device verification.
- **The Windows named-pipe transport** — `service::transport::
  windows.rs` is a **real, complete implementation** (not a stub),
  written to type-check via CI's cross-compile check. It has never run
  against a real client on real Windows — that's the actual remaining
  work, not writing it.

## 2. What needs real per-OS work (named APIs, not guesses)

Both `platform::{macos,windows}::mod.rs` stub every `PlatformAdapter`
method with `Err(Unsupported("... see unit U12"))`, but their own
comments already name the target APIs — this is design work already
done, just not implemented:

| Capability | macOS target API | Windows target API |
|---|---|---|
| Process priority | `setpriority(2)` (same POSIX call Linux already uses) | `SetPriorityClass`/`GetPriorityClass` (TRS §38 names this explicitly) |
| CPU affinity | `thread_policy_set`/taskpolicy-style tagging (no direct `sched_setaffinity` equivalent — a real API-shape difference, not just naming) | `SetProcessAffinityMask`/`GetProcessAffinityMask` (TRS §38's other named API) |
| Suspend/resume | `kill(2)` + SIGSTOP/SIGCONT (same as Linux) | Per-thread `SuspendThread`/`ResumeThread` — Windows has **no process-level SIGSTOP equivalent**, a genuine design difference to account for, not an implementation detail |
| Self-process metrics | not yet named in comments — likely `proc_pid_rusage`/`task_info` (libproc) | not yet named — likely `GetProcessTimes`/`GetProcessMemoryInfo` |
| Host telemetry (CPU/memory/network/storage/process enumeration) | **No module exists at all yet.** TRS §39 names: `libproc`, `host_statistics64`, IOKit, an `SMAppService` privileged helper for anything needing elevation | **No module exists at all yet.** TRS §38 names: Performance counters/WMI |
| `std::process::Command` action surface | `platform/macos/exec.rs` exists, empty | `platform/windows/exec.rs` exists, empty |

Two platform-specific constraints already named in the spec, worth
knowing before starting:

- **macOS**: TCC-denied permissions must report `PermissionRequired`
  with a pointer to the relevant System Settings pane, never fail
  silently. Endpoint Security and Network Extension are explicitly
  out of scope for v1.x (both require Apple-granted entitlements).
- **Windows**: a Windows Service host for the core is named in TRS
  §38 as part of the real deployment shape (not just a console app).
  ETW and the Windows Filtering Platform are explicitly deferred past
  v1.x.

## 3. Reusable-for-both list (things to build once, use on both OSes)

- A `ProcSource`-equivalent abstraction per OS — Linux's own
  `ProcSource` trait pattern (one `read(path) -> String` seam,
  fixture-testable) is a good template to replicate structurally for
  macOS/Windows telemetry, even though the concrete syscalls differ
  entirely.
- The `PlatformAdapter` trait itself needs no changes — both new
  implementations slot into the exact same interface Linux already
  satisfies.
- `core::actions::host::{priority,affinity}`'s `ActionKind`
  implementations are already OS-agnostic at the type level (they call
  through `PlatformAdapter`); once a real macOS/Windows
  `PlatformAdapter` exists, these likely need zero changes.
- The Flutter console's `flutter run -d macos` / `flutter run -d
  windows` targets — no console code changes anticipated; verification
  work only (fonts, window chrome, file-path conventions).
- Credential storage, TLS, and the whole DBMS/analysis/correlation/AI
  pipeline — build once (already done), verify on-device per OS.

## 4. macOS-specific prompt/TODO list

1. Design and implement `platform::macos`'s real `PlatformAdapter` —
   start with `self_process_metrics` (needed for `/health` to report
   anything real), then process priority/CPU affinity/suspend-resume
   in the order the action registry actually needs them.
2. Build a macOS telemetry module (new — no existing skeleton) using
   `libproc`/`host_statistics64`/IOKit per TRS §39; mirror Linux's
   `ProcSource`-style seam so `core::telemetry`'s existing parser
   pattern can be followed rather than reinvented per-metric.
3. Design the `SMAppService` privileged-helper boundary for whichever
   actions need elevation (this is new architecture, not present on
   Linux at all — Linux's actions run as the invoking user).
4. Wire TCC-permission-denied paths to `CapabilityError::
   PermissionRequired` with a real System Settings pane pointer, per
   TRS §39 — audit every macOS syscall call site for this once written.
5. Verify the OS credential store (`keyring` crate → Keychain) actually
   round-trips a real password on real macOS hardware.
6. `flutter run -d macos` — confirm the console builds/runs without
   code changes; check window chrome, fonts, and the GTK-analogous
   desktop-toolchain prerequisites for macOS specifically.
7. Decide the macOS service-hosting story (launchd agent/daemon?) —
   analogous to the Windows Service question in TRS §38, not yet
   addressed for macOS in any ADR.
8. Re-run the full Linux test plan's automated suite (§2 of the Linux
   Test Plan doc) on macOS once `cargo check --workspace` becomes
   `cargo test --workspace` there — expect the OS-agnostic crates'
   tests to pass unchanged; only newly-added macOS-specific tests are
   new work.

## 5. Windows-specific prompt/TODO list

1. Design and implement `platform::windows`'s real `PlatformAdapter` —
   `SetPriorityClass`/`SetProcessAffinityMask` first (both explicitly
   named in TRS §38, so the "what API" question is already answered);
   `self_process_metrics` via `GetProcessTimes`/
   `GetProcessMemoryInfo`.
2. Design suspend/resume carefully — Windows has no process-level
   SIGSTOP. Decide: enumerate and suspend every thread in the target
   process (`SuspendThread` per thread), or use Job Objects (TRS §38
   names Job Objects for process priority — check whether they also
   offer a cleaner suspend primitive) before writing code.
3. Build a Windows telemetry module (new) using Performance Counters/
   WMI per TRS §38; same "mirror the `ProcSource` seam" recommendation
   as macOS.
4. **Verify the named-pipe transport for real** —
   `service::transport::windows.rs` is written and compiles, but has
   never been exercised against a real client. This is likely the
   single fastest, highest-value Windows verification task: build for
   real on a Windows box, start the service, connect a real named-pipe
   client (or point the console's future Windows transport at it), and
   confirm the same request/response cycle Linux's Unix-socket
   transport already proves in tests.
5. Design the Windows Service host (TRS §38's own named requirement) —
   install/start/stop lifecycle, likely via the `windows-service` crate
   or equivalent; not started anywhere in the codebase yet.
6. Build the console's Windows-side `LocalTransport` implementation
   (named pipes) — currently only `UnixSocketTransport` exists;
   `LocalTransport` is already an abstract interface designed for this
   second implementation.
7. `flutter run -d windows` — confirm the console builds/runs; check
   the Windows-equivalent desktop-toolchain prerequisites (Visual
   Studio Build Tools, not GTK).
8. Verify the OS credential store (`keyring` crate → Windows Credential
   Manager) round-trips a real password on real Windows hardware.
9. Re-run the automated test suite on Windows once real
   implementations exist — same expectation as macOS: OS-agnostic
   crates should already pass.

## 6. Current feature list

**Host telemetry** (Linux only, real): CPU/memory/storage/network/
process/device snapshots with deterministic 4-tier pressure
classification and background-vs-foreground process attribution;
historical rollups (raw/hourly/daily).

**PostgreSQL observability** (cross-platform, real): sessions, locks,
query/table/index stats, replication status, GUCs, temp-file activity,
deadlocks, long transactions, idle-in-transaction sessions; TLS
Disable/Prefer/Require/VerifyCa/VerifyFull; least-privilege role
enforcement.

**Deterministic host+DB analysis and cross-layer correlation**
(cross-platform business logic; needs real Linux telemetry input to
run end-to-end today): 11 DB health categories, ~10 host bottleneck
types, 9-cause correlation engine, provably never AI-dependent.

**Optional AI explanation layer** (cross-platform): natural-language
explanation of any already-computed verdict, schema-validated by
citation against the real evidence sent, gated behind a configured
Ollama endpoint.

**Policy-gated action execution** (Linux only, real; two action
types): process priority, CPU affinity — both with real approval
workflow, rollback, and resumption.

**Security suspend/resume response** (Linux only, real): SIGSTOP/
SIGCONT via the same policy-gated pipeline.

**Eight DB security detectors** (cross-platform business logic, real):
superuser override, untrusted device, GUC change, role-superuser
grant, unusual client address, role-membership grant, table-privilege
grant, authentication failure — correlated into incidents,
suppressible, closeable, with real repetition-based severity
escalation.

**Tuning engine** (Linux only, real, needs real process actions to
apply): three automation modes, statistical before/after verification
via Mann-Whitney U.

**Full Flutter console** (cross-platform UI code, currently verified
on Linux only): every domain above has a real screen; thin,
non-reclassifying client per FR-CONSOLE-001.

## 7. Wishlist for further enhancement

- **v2 fleet coordinator** — the big one, already scoped in ADR 0071
  as a design proposal; the natural "what's next after Mac/Windows"
  item.
- **macOS/Windows telemetry, actions, and transport** — the whole of
  §2/§4/§5 above.
- **`ResourceDescriptor` glob/pattern matching** for protected
  resources (currently exact-name-only, ADR-flagged as a future widen
  point) — security-sensitive, worth real design attention before
  building.
- **Automatic incident resolution** — explicitly named and explicitly
  deferred (ADR 0081) in favor of manual-only closing; revisit if
  manual closing proves too noisy in practice.
- **A fourth demo scenario** (network-latency or replication-lag) to
  extend the existing three-scenario end-to-end correlation proof
  harness.
- **Multi-DBMS support beyond PostgreSQL** — explicitly out of scope
  for v1 (SRS §2); a real future-version question, not a v1 gap.
- **A real console-side Windows named-pipe `LocalTransport`** and
  (per §5.6 above) whatever the equivalent macOS transport question
  turns out to be — Flutter's Unix-socket path may or may not need a
  distinct macOS implementation; verify before assuming it's a
  Windows-only gap.
- **`README.md` refresh** — still says "unit U1 of 16"; a good first
  task for whoever opens this repo next, on any OS.
