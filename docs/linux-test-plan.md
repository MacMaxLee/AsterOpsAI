# AsterOpsAI Linux Test Plan

A concrete, step-by-step guide for verifying an AsterOpsAI checkout on
Linux: automated tests, every CI gate reproduced locally, a manual
smoke test of the running service + console, and the existing demo
scenarios that prove the correlation engine end-to-end. Every command
below is copy-pasteable from the repo root unless noted.

## 1. Prerequisites

| Need | Why | Install |
|---|---|---|
| Rust stable toolchain | builds `rust_core`'s four crates | `rustup` if not already present |
| Flutter stable channel | builds/tests the console | `subosito/flutter-action`-equivalent locally, or your distro's Flutter SDK |
| Real PostgreSQL 13/15/17 binaries | `core`'s `dbms_*` integration tests spin up real, throwaway PostgreSQL instances — no Docker, no system install | `bash scripts/setup-test-postgres.sh` (extracts official PGDG `.deb`s into `~/.local-tools/pgNN/`, no root, idempotent) |
| `clang cmake ninja-build pkg-config libgtk-3-dev` | only needed to *run* the console as a real Linux desktop app (`flutter run -d linux`) or the demo scenarios — not needed for `flutter test`/`flutter analyze` | `sudo apt-get install -y clang cmake ninja-build pkg-config libgtk-3-dev` |
| `openssl` CLI | several `core` TLS tests generate real self-signed certs | present on virtually every Linux dev box already |

Run `bash scripts/setup-test-postgres.sh` once before anything else —
almost every Rust integration test below depends on it.

## 2. Automated test suite

### 2.1 Rust — the primary suite

```sh
cd rust_core
cargo test --workspace --all-features
```

This runs every crate's unit tests (inline `#[cfg(test)] mod tests`)
and every `tests/*.rs` integration test across `contracts`,
`platform`, `core`, and `service`. Expect this to take a few minutes
the first time (each `dbms_*`/`security_detector_*` test spins up a
real, disposable PostgreSQL instance via `initdb`/`pg_ctl`) and to run
many instances **in parallel** — this is by design (each test gets its
own throwaway data directory and port), but it does mean the suite is
CPU- and disk-hungry. If you see an intermittent timing-sensitive
failure under heavy parallel load that doesn't reproduce alone, that's
a known category (see ADR 0085) — rerun the single failing test file
alone to confirm before treating it as a real regression:

```sh
cargo test -p <core|service> --test <failing_test_file>
```

### 2.2 Console — Flutter

```sh
cd console
flutter pub get
flutter test
```

Runs every widget test under `console/test/` (one per screen, plus a
`FakeTransport`-scripted test per mutation/error state, plus two
`meetsGuideline` accessibility checks). No GTK/desktop toolchain
needed for this — it uses Flutter's headless test harness.

A separate **live integration test** exists and is *not* run by
`flutter test`:

```sh
cd rust_core && cargo build --workspace && cd ..
cd console && dart run integration/reconnect_live_test.dart
```

This starts the real `ai-ops-core` binary, kills it mid-session, and
confirms the console's own connection-state machine notices and
recovers — a real process-lifecycle test, not simulated.

## 3. Every CI gate, reproduced locally

CI (`.github/workflows/ci.yml`) runs 16 independent jobs on every push
to `main` and every PR. All are required; none are advisory. Run them
all locally before pushing to catch anything CI would catch:

```sh
# Formatting
cargo fmt --all -- --check

# Lints, workspace-wide, warnings as errors
cd rust_core && cargo clippy --workspace --all-targets --all-features -- -D warnings && cd ..

# The full test suite (see §2.1)
cd rust_core && cargo test --workspace --all-features && cd ..

# Dependency policy (licenses, bans, duplicate-version bloat, security advisories)
cargo deny check          # needs `cargo install cargo-deny` once

# Known-vulnerability advisory scan (a subset of what cargo-deny also checks)
cargo audit                # needs `cargo install cargo-audit` once

# contracts' emitted JSON schemas match what's committed under /schemas
bash scripts/check-schema-drift.sh

# The two structural grep gates (not style nits — CI-enforced architecture rules)
bash scripts/check-no-command-outside-exec.sh   # std::process::Command only inside platform/*/exec.rs
bash scripts/check-no-sql-outside-adapters.sh   # SQL literals only inside core/{dbms/adapters,repository}/* or */tests/

# Every SRS FR-ID has a real, cited implementation + test (docs/traceability-matrix.md)
bash scripts/check-fr-id-traceability.sh
# (regenerate first if you added/removed any FR-ID citation:)
bash scripts/generate-traceability-matrix.sh

# TRS §20 / SRS FR-PERF-005: no AI-module item reachable from analysis/correlation
# needs: cargo install cargo-modules --locked --version 0.27.0
bash scripts/check-no-ai-reachable-from-analysis-or-correlation.sh

# Cross-platform compile checks (type-check only, not run — see §5 below)
cd rust_core && cargo check --workspace && cd ..   # run once per target if you have the toolchains:
# cargo check --workspace --target x86_64-pc-windows-msvc
# cargo check --workspace --target aarch64-apple-darwin

# Console gates
cd console
flutter analyze
dart format --output=none --set-exit-if-changed .
flutter test                                          # same as §2.2
cd ..
bash scripts/check-console-codegen-drift.sh            # generated/models/*.dart matches a fresh `dart run tool/generate_models.dart`
```

If every one of the above passes, CI will pass — this is the exact set
of jobs it runs, not an approximation.

## 4. Manual smoke test (real service, real console)

1. **Build and start the service:**
   ```sh
   cd rust_core
   cargo build --workspace
   cargo run -p service
   ```
   It listens on a Unix Domain Socket
   (`$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`), never TCP (ADR
   0001).

2. **Confirm it's alive:**
   ```sh
   curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
     http://localhost/api/v1/health
   ```
   Expect a `{"success":true,...}` envelope with a real capability map.

3. **Poke a few more real endpoints** (still no DB configured — these
   should all return real Linux telemetry):
   ```sh
   curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" http://localhost/api/v1/cpu
   curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" http://localhost/api/v1/memory
   curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" http://localhost/api/v1/processes
   ```

4. **Point it at a real PostgreSQL instance** (optional, but exercises
   the entire `dbms`/security-detector/AI-explanation surface) by
   setting `ASTEROPS_DB_HOST`/`ASTEROPS_DB_PASSWORD` (+ `_PORT`/
   `_DATABASE`/`_USER`/`_SSLMODE`/`_CA_BUNDLE_PATH` as needed — see
   `service::dbms_config` for the full list) before step 1, then:
   ```sh
   curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" http://localhost/api/v1/dbms/sessions
   ```

5. **Run the console against it:**
   ```sh
   cd console
   flutter run -d linux
   ```
   (needs the GTK/desktop toolchain from §1). Confirm the dashboard
   shows live-updating telemetry and that navigating to every screen
   (CPU/Memory/Storage/Network/Processes/Devices, Database, Security
   Incidents, Policy Inbox, Tuning History, Host/DB/Correlation
   analysis) loads without a connection error.

## 5. Cross-platform build checks (not functional tests)

`check-windows`/`check-macos` in CI run `cargo check --workspace` on
real `windows-latest`/`macos-latest` (arm64) runners — **type-checking
only**, proving the whole workspace still compiles for those targets
(the `PlatformAdapter` stubs, the real named-pipe Windows transport,
etc.), not that any of it actually *works* there. There is currently no
automated functional test for macOS or Windows — see the separate
macOS/Windows transition documents for what real verification on those
platforms would need.

## 6. End-to-end scenario verification (the demo harness)

`docs/demo/RUNBOOK.md` already documents this in full; summarized here
so this test plan is self-contained:

```sh
bash scripts/setup-test-postgres.sh                       # if not already done
cd rust_core && cargo build --release -p service && cd ..
scripts/demo/build-console.sh                              # needs the GTK toolchain from §1
sudo scripts/demo/setup-io-cgroup.sh "$USER"                # optional, storage-latency only
scripts/demo/lock-storm.sh
scripts/demo/pool-exhaustion.sh
scripts/demo/storage-latency.sh
scripts/demo/reset.sh                                       # tear down between/after runs
```

Each scenario provisions a real, throwaway PostgreSQL instance,
induces a **real** condition (an actual blocking lock chain, actual
connection-pool saturation, actual kernel-throttled disk I/O — never a
simulated/mocked signal), starts the real service + console pointed at
it, and checks the resulting cross-layer correlation verdict against a
written `expected-verdict.json`. This is the strongest end-to-end proof
in the repo that host+DB correlation actually works, not just that its
unit tests pass in isolation.

Known caveat (documented in the runbook, not a bug to chase): on fast
virtualized/SSD-backed storage, `storage-latency.sh`'s unprivileged
`dd`-based contention alone may not reliably cross the real latency
thresholds — the `setup-io-cgroup.sh` delegation step exists
specifically to make that scenario reliable on such hardware via a
genuine kernel-level `io.max` cap.

## 7. What real hardware adds beyond this sandbox

Everything above has been run in a sandboxed dev container. Real
next-step verification, once available (a dedicated Ubuntu server is a
good target): confirm the full suite (§2–§4) behaves identically on
bare metal — particularly `platform::linux::process_control`'s real
`setpriority(2)`/`sched_setaffinity(2)`/`SIGSTOP`/`SIGCONT` calls (the
sandbox's own process-control tests already exercise these for real,
but a dedicated box is worth a second confirmation pass, plus a look at
whether cgroup v2 delegation for the storage-latency demo behaves
differently outside a container), and a longer-running soak of
`self_metrics`/`retention` background schedulers than a CI run ever
exercises.
