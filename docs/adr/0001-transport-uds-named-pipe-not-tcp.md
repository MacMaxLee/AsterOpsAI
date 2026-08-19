# 0001 — Local API Transport: Unix Domain Socket / Named Pipe, Not Loopback TCP

## Status

Accepted (unit U0).

## Context

The core service exposes a local API that the console, and later the AI
explanation layer's supporting tooling, must reach. SRS FR-SYS-001 requires
this API not be reachable from the network, or from other local users on a
shared machine, by default. A loopback TCP socket (`127.0.0.1:PORT`) is
reachable by any local process or user unless additional authentication is
layered on top — it does not satisfy this requirement on its own.

## Decision

The core serves its API over a Unix Domain Socket on Linux/macOS, at
`$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`, mode `0600` (owner
read/write only), and over a Named Pipe on Windows at
`\\.\pipe\ai-ops-coordinator\core`. No TCP fallback is offered, ever — this is
a structural choice, not a default that can be reconfigured away.
`$XDG_RUNTIME_DIR` being unset is a hard startup failure, not a silent
fallback to a less-safe location such as `/tmp`.

## Consequences

- The console needs a UDS/named-pipe-aware HTTP client rather than a plain
  loopback `http://` base URL; this is scoped into unit U3.
- Manual testing and the U0 Definition of Done use
  `curl --unix-socket <path> http://localhost/api/v1/health` rather than a
  normal URL.
- `rust_core/service/src/transport/` carries real platform branching
  (`unix.rs` / `windows.rs`) that a TCP-only design would not have needed.
- A future requirement to expose the API beyond the local machine (e.g. for
  the v2 agent/coordinator split) is deliberately a *different* transport
  (mTLS, per TRS §30) layered alongside this one, not a loosening of it.
