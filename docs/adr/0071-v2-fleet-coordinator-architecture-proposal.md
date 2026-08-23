# 0071 — v2 Fleet & Coordinator Architecture: A Design Proposal

## Status

**Proposed** — design and phasing only, no implementation unit yet, no
code in this change. Unlike every other ADR in this repo (each
retrospectively documents a completed, `Accepted` unit), this one exists
to make whichever future unit actually builds this start from a
concrete plan instead of a blank page. Do not treat anything here as
`Accepted` until a real unit implements it and updates this status.

## Context

SRS §35 (FR-FLEET-001/002/003) and CLAUDE.md itself both explicitly
frame the fleet coordinator as v2 scope that doesn't exist in this repo
yet — ADR 0066's audit named all three FR-FLEET-* IDs as a deliberate,
allowlisted exception to the traceability gate, not a v1 gap. TRS
already commits to real, specific decisions ahead of any implementation
(§2, §28–§32, §43, §46) — this proposal's job is to organize those into
an actionable plan and surface what TRS leaves genuinely open, not to
invent new architecture wholesale.

**What TRS already decides** (cited, not restated in full):
- Two new crates, `rust_core/agent` and `rust_core/coordinator` (§28–29),
  reusing `core`/`platform` directly — the agent has no dependency on
  the console.
- A deliberate storage asymmetry: agent-local SQLite, coordinator
  PostgreSQL (§29) — the coordinator is the fleet's shared system of
  record.
- mTLS agent↔coordinator, per-agent identity, cert rotation; enrollment
  via short-lived join-token → CSR → coordinator-signed cert; nonce+
  timestamp replay protection (§30).
- Store-and-forward: local buffering while disconnected, bounded-ring
  backfill on reconnect, oldest-first drop with an explicit recorded gap
  — never a silently closed hole (§31).
- Coordinator schema mirrors agent domain tables plus fleet-specific
  ones: inventory, enrollment, certificate state, connectivity history
  (§32).
- Automatic replica promotion/failover are permanently out of scope,
  full stop (§42, FR-FLEET-002) — not deferred, never planned.
- Three RBAC roles, server-side enforced on every coordinator request:
  VIEWER, OPERATOR, ADMIN (§46, FR-FLEET-003).
- v1.x keeps working standalone; the coordinator is never required for
  single-machine functionality (§43).

**A real, concrete reuse opportunity found while researching this**:
the agent doesn't need a new persistence layer at all. `core::repository`
already *is* a single-writer-thread SQLite store (ADR 0007) with a
telemetry-snapshot persistence path
(`try_persist_telemetry_snapshot`) — the agent can depend on `core`
and use this exact module for its local buffer. The only genuinely new
storage piece is tracking *how far the coordinator has acknowledged*,
not a second copy of the telemetry data.

## Decision (proposed)

### Crate boundaries

- `rust_core/agent`: a headless binary. Depends on `contracts`,
  `platform`, `core` (same alias convention as `service`:
  `ai_ops_core = { package = "core", ... }`). Reuses `core`'s telemetry
  sampling and DBMS-adapter code directly — the same capability model
  `service` already uses, not a reimplementation. Runs its own local
  `core::repository` SQLite instance for buffering (see below). Whether
  it also exposes a local UDS API (for local CLI/diagnostics, mirroring
  `service`) or is push-only is an open question (see below).
- `rust_core/coordinator`: a separate binary/service. Depends on
  `contracts` for wire types shared with agents, but its own persistence
  is PostgreSQL, not `core::repository` (SQLite) — a genuinely different
  storage layer, most naturally built directly on `tokio_postgres` /
  `deadpool_postgres` (already workspace dependencies, used today for
  *monitoring target* Postgres instances — reusing the same driver for
  the coordinator's *own* storage, not a new one). Unlike the agent's
  single-writer SQLite pattern (ADR 0007, needed because SQLite has no
  real concurrent-writer story), Postgres handles concurrent writers
  natively, so the coordinator does not need to replicate that
  architecture — a normal connection pool is sufficient.
- Workspace `members` gets these as one-line additions (`Cargo.toml`
  already anticipates this — see the fix alongside this ADR removing a
  stale `# U13 adds: ...` comment left over from ADR 0003, since unit
  numbering has since moved on and U13 was reused for unrelated work).

### Agent-local buffering (closes the FR-FLEET-001 "backfilling without
unbounded local growth" half)

Reuse `core::repository`'s existing tables as the buffer itself — no
second, duplicate outbox table. New: a small `coordinator_sync_state`
table (one row per domain table: telemetry snapshots, performance
analysis, security incidents, actions, audit) tracking the last
row id/timestamp the coordinator has acknowledged. Backfill on
reconnect reads forward from that watermark; a bounded row-count or
age-based cap per domain table, enforced the same way `core`'s existing
7-day retention already caps growth (TRS §45's own retention
precedent), triggers the "oldest dropped, gap explicitly recorded"
behavior TRS §31 requires — likely a new `buffer_gap` row type rather
than silently advancing the watermark past missing data.

### Agent↔coordinator protocol

TRS §30 fixes the transport (mTLS) and the replay-protection shape
(nonce+timestamp), but not the wire format for the actual data push.
**Proposed**: keep the existing contract-first JSON convention
(`contracts` crate, serde+schemars, `/schemas/*.schema.json`) rather
than introducing a second format (e.g. protobuf/gRPC) — new
fleet-specific contract types (agent inventory record, a batched
telemetry-rollup push, enrollment request/response) generated and
schema-drift-checked exactly like every existing wire type. This avoids
a second, parallel serialization toolchain for what both TRS and this
project's own conventions already treat as a solved problem.

### RBAC (closes FR-FLEET-003)

TRS §46 fixes the three roles and "server-side, every request, console/
agent never a security boundary." **Open, not resolved here**: mTLS
authenticates *agents*; it says nothing about how a *human operator*
authenticates to the coordinator to be assigned a role in the first
place. This is a real, separate trust boundary TRS doesn't address, and
a future unit needs to either extend the existing Flutter console with
a coordinator-auth flow (session tokens? its own login?) or introduce a
distinct fleet-console surface. Naming this explicitly rather than
assuming an answer.

### FR-FLEET-002 (no auto-promotion/failover) as an early, cheap
guarantee

This doesn't need the full fleet stack to start proving. Once *any*
coordinator-side action registry exists, the same closed-enum pattern
`security::response::SecurityResponseAction` already establishes
(ADR/unit U11, reinforced by unit U64's pin test) applies directly: no
variant for replica promotion or failover ever gets added, and a pin
test over the registry's enum proves it stays that way. Worth doing as
the *first* real coordinator-side test, before the harder mTLS/RBAC
work, since it's cheap and closes a real FR-ID immediately once the
crate exists.

## Proposed phasing (a sequence, not committed unit numbers)

1. `rust_core/agent` crate skeleton: headless daemon wrapping `core`'s
   existing sampling loop, writing into its own local
   `core::repository` SQLite file, zero networking. Proves the
   "operates headlessly, continues buffering" half of FR-FLEET-001 in
   isolation.
2. `rust_core/coordinator` crate skeleton: PostgreSQL schema (fleet
   inventory, enrollment, cert state, connectivity history per TRS
   §32), minimal HTTP surface (health check only). The FR-FLEET-002
   pin test (above) lands here, immediately.
3. Enrollment protocol: join-token issuance, CSR exchange, coordinator-
   signs, cert rotation. This is the highest-risk, most security-
   sensitive phase — likely deserves its own dedicated ADR when it's
   actually built, not just this proposal's sketch.
4. mTLS transport wiring both directions + the actual telemetry-push/
   backfill protocol (`coordinator_sync_state` watermarks, bounded
   buffer, explicit gap recording). Completes FR-FLEET-001.
5. RBAC roles + server-side enforcement, once the human-operator-auth
   open question above has its own real answer. Completes FR-FLEET-003.

## Consequences

- This ADR is a planning artifact, not a traceability-matrix entry —
  `scripts/generate-traceability-matrix.sh` still correctly reports
  FR-FLEET-001/002/003 as `MISSING` (allowlisted) until a real phase
  above actually ships code and tests.
- `Cargo.toml`'s stale `# U13 adds: ...` comment is fixed alongside this
  ADR to stop pointing at a unit number that's long since been reused.
- The next unit to touch fleet work should start at phase 1 above and
  either update this ADR's `Status` to `Accepted` with real
  consequences, or write a new ADR that supersedes specific decisions
  here — not silently diverge from what's written.
