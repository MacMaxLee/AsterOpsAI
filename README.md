# AsterOpsAI

Status: Milestone 4 complete — macOS platform support shipped. Milestone 5 (documentation) in progress.

A host + PostgreSQL observability tool: a Rust core service, a thin console
client, and (v2) a fleet coordinator. Answers "is something wrong?" and "is
it the database, or is it the box?" with evidence, not guesses.

- [`docs/SRS.md`](docs/SRS.md) — functional requirements
- [`docs/TRS.md`](docs/TRS.md) — technical requirements
- [`docs/adr/`](docs/adr/) — architecture decision records
- [`CLAUDE.md`](CLAUDE.md) — how this repo is developed, unit by unit

## Build & run

```sh
cargo build --workspace
cargo run -p service
```

The service listens on a Unix Domain Socket (Linux/macOS) or Named Pipe
(Windows) — not TCP, by design (see
[docs/adr/0001-transport-uds-named-pipe-not-tcp.md](docs/adr/0001-transport-uds-named-pipe-not-tcp.md)).
With the service running:

```sh
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/health
```

Real host telemetry is available on Linux and macOS: `/api/v1/cpu`, `/memory`,
`/storage`, `/network`, `/processes`, `/devices`, `/system/status`.

## macOS Quick Start

On macOS, you need to set `XDG_RUNTIME_DIR` for the Unix socket:

```sh
export XDG_RUNTIME_DIR="/tmp/runtime-$(id -u)"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

# Build and run
cargo build --release
./target/release/ai-ops-core serve
```

Test with curl (in another terminal):
```sh
curl --unix-socket "$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock" \
  http://localhost/api/v1/health
```

For complete installation instructions, prerequisites, Flutter console setup, and troubleshooting, see **[`docs/INSTALL-MACOS.md`](docs/INSTALL-MACOS.md)**.

## Platform support

| Platform | Build | Tests | Telemetry | Status |
|----------|-------|-------|-----------|--------|
| Linux | ✅ Full | ✅ Full suite | ✅ Complete | Production-ready |
| macOS | ✅ Full | ✅ Library tests | ✅ Complete* | Verified in CI |
| Windows | ✅ Check only | ❌ None | ❌ Stubbed | Compilation only |

*See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for macOS-specific gaps (CPU affinity unsupported per ADR 0086)

## Test & lint

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
bash scripts/check-no-command-outside-exec.sh
bash scripts/check-no-sql-outside-adapters.sh
bash scripts/check-schema-drift.sh
```
