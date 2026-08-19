# 0009 — PostgreSQL Adapter Design: TLS Connector, Test Harness, and Schema

## Status

Accepted (unit U4).

## Context

U4 adds the first `DbmsAdapter` implementation (PostgreSQL, read-only, TRS
§33's exact 13-method set). Several concrete design questions came up that
weren't fully resolved by SRS §26-§34 / TRS §17-§19 alone, each settled by
real research or real testing rather than guessed — recorded here so the
reasoning survives past this session.

## Decision

**TLS**: `tokio-postgres` is connector-agnostic — `TlsMode` (this unit's
enum) mirrors `tokio_postgres::config::SslMode` exactly: `Disable`/
`Prefer`/`Require` only. `verify-ca`/`verify-full` are **not** implemented
this unit — they need a CA-bundle path `ConnectionMetadata` doesn't carry
yet, plus a certificate-verifying connector on top of what's wired up here;
real, separate scope, not something to fake by adding enum variants this
code can't honor. For `Require`, real TLS 1.2/1.3 handshake signature
verification runs for real (via `rustls::crypto::{verify_tls12_signature,
verify_tls13_signature}`, the crate's own helpers — not skipped), while
certificate-*chain* verification is intentionally skipped: this is
`sslmode=require`'s actual, documented PostgreSQL semantics ("encrypt, but
don't verify the server's identity"), not a shortcut invented here.
`rustls`'s `ring` crypto provider is used instead of the default
`aws_lc_rs` because `aws_lc_rs` needs `cmake` to build its C library from
source, which this dev sandbox doesn't have (same class of gap as U3's
missing GTK toolchain; `ring` only needs a plain C compiler, already
proven working via U2's `rusqlite` `bundled` feature). Verified for real:
`dbms_tls_require_test.rs` starts a real PostgreSQL instance with a real
self-signed cert and confirms — via the server's own `pg_stat_ssl` view,
not just "connect didn't error" — that a `Require`-mode connection is
actually encrypted.

**Real PostgreSQL instances for tests, not Docker — and not GitHub Actions'
`services:` containers either, even in CI.** This sandbox has no Docker.
PGDG's official `.deb` packages (apt.postgresql.org) extract via `dpkg -x`
(no `dpkg -i`, no apt, no root) into `~/.local-tools/pgNN/`, giving real
`postgres`/`initdb`/`pg_ctl`/`pg_basebackup`/`psql` binaries runnable as an
unprivileged user — confirmed working end-to-end (PG 13, 15, and 17 each
extracted, `initdb`'d, started, connected to, stopped cleanly) before any
adapter code was written. `scripts/setup-test-postgres.sh` scripts exactly
those proven steps. A GitHub Actions `services:` container was the first
design considered for CI, and rejected once the test suite's actual needs
became clear: a `services:` container is a single, already-running,
passive instance the job connects *to* — it can't be `pg_basebackup`'d
into a second instance (the replica test), killed and restarted on the
same data directory (the mid-poll-disconnect test), or started with a
custom self-signed cert already in place (the TLS test). CI instead runs
the exact same `scripts/setup-test-postgres.sh` + extracted-binary harness
as local development — one mechanism, proven to work locally first, reused
identically in CI, rather than two different approaches that would need to
be verified (and kept in sync) separately. All 21 `dbms_*` integration
tests run against genuinely real server processes: 13/15/17 version smoke
tests, a real primary + streaming-replica standby (`pg_basebackup -R`, the
documented way to do it, not a hand-rolled recovery-config
approximation), a real `SIGTERM`-killed-and-restarted server for the
mid-poll-disconnect case, a real `REVOKE`d role for the permission-denied
case, and the real OS credential store (Secret Service, confirmed live on
this session's D-Bus) for the credential-store round-trip.

**`core::dbms::capability::Gated<T>`, not `contracts::Capability`.**
`contracts::Capability` (reused everywhere else in this codebase for the
Supported/Limited/Unavailable/PermissionRequired state family) carries no
payload — there's nowhere to put `query_stats()`'s `Vec<QueryStat>` on the
`Supported` case. `Gated<T>` is the same four-state shape with a payload
added; kept crate-internal (`core::dbms`, not `contracts`) since there's no
API layer consuming it yet (see the plan's SCOPE note — this unit is the
collection/storage layer only, not wired into `service` or the console).

**SQL privacy's real raw-SQL source is `pg_stat_activity`, not
`pg_stat_statements`.** Originally assumed (before checking) that
`query_stats()`'s `QueryStat` would carry an opt-in `raw_query` field.
Real research: `pg_stat_statements.query` is *always* pre-normalized by
PostgreSQL itself — there is no raw form available from that source at
all. `SessionInfo.query`/`LongTransaction.query` (from `pg_stat_activity`,
which does carry verbatim query text) are the actual `capture_raw_sql`-
gated fields; `core::dbms::privacy::sanitize_query` redacts
credential-shaped substrings unconditionally and normalizes (literal
substitution, not `pg_stat_statements`'s exact algorithm, but the same
spirit) unless the opt-in is set.

**Migration V8 drops V7's `dbms_metrics` placeholder** rather than leaving
it alongside the real schema — it was never written to or read from by any
shipped code (only asserted to exist by the migration-list test), and
V7's own comment already named this unit as the one that would supersede
it. Replaced with `dbms_connections` (connection metadata, no password
column — TRS §18/SRS NFR-PRIV-002 enforced by the type, not the schema)
and one wide `dbms_snapshots` table mirroring `telemetry_snapshots`'s own
shape (V1, unit U2): scalar per-instance fields get real columns,
every variable-cardinality result (sessions, locks, table/index stats,
replication standbys, GUCs, long transactions, idle-in-transaction
sessions) is a JSON column — not twelve separate normalized tables for
data nothing writes to yet. No code populates this table this unit (the
live poll loop that would is later-unit work per the SCOPE note); the
schema exists now because the adapter's real return shapes are already
fixed, matching V4/V5/V6's own "schema now, writer later" precedent.

## Consequences

- `TlsMode::VerifyCa`/`VerifyFull` are a real, flagged gap: a future unit
  wiring up production PostgreSQL connections with real CA-verified TLS
  needs to add a CA-bundle field to `ConnectionMetadata` and a
  certificate-verifying connector — not assume `Require` already covers it.
- `refinery::embed_migrations!` doesn't reliably make Cargo detect a *new*
  file appearing in the migrations directory (only changes to
  already-tracked files) — a real build-cache gotcha hit while adding V8,
  documented directly in `core/src/repository/migrations.rs`. Harmless for
  a fresh clone or CI; a local incremental build that seems to "forget" a
  new migration just needs a `touch`/`cargo clean -p core`, not a code fix.
- `core/tests/dbms/common/mod.rs`'s real-PostgreSQL harness needs
  `scripts/setup-test-postgres.sh` run once per machine (idempotent,
  skips already-present versions) — including in CI, as an explicit step
  before `cargo test`, since a fresh `ubuntu-latest` runner has nothing
  cached from a prior run.
- Both the SQL-outside-adapters and no-Command-outside-exec grep gates
  gained a `core/tests/` exception this unit (the harness spawns/kills
  real PostgreSQL processes and, incidentally in one already-existing
  case, issues raw SQL to inspect fixture state) — the same precedent the
  SQL gate already established for `core/tests/` during U2's bugfix pass,
  now extended to the Command gate for the same reason: test infrastructure
  standing up a throwaway dependency is a different concern from the
  product's own runtime subprocess execution, which is what that gate
  exists to keep centralized and auditable.
