# AsterOpsAI

Status: bootstrapping — unit U1 of 16.

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

On Linux, real host telemetry is also available: `/api/v1/cpu`, `/memory`,
`/storage`, `/network`, `/processes`, `/devices`, `/system/status`.

## Test & lint

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
bash scripts/check-no-command-outside-exec.sh
bash scripts/check-no-sql-outside-adapters.sh
bash scripts/check-schema-drift.sh
```
