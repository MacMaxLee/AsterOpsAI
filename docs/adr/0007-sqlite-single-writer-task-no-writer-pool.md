# 0007 — SQLite Single Writer Task, No Writer Pool

## Status

Accepted (unit U2).

## Context

U2 adds SQLite-backed persistence (`core::repository`) behind rusqlite, which
is synchronous/blocking — every call it makes is a blocking OS call, unlike
everything else `core`/`service` talk to so far. U0/U1 established two
`tokio::spawn` + `Arc<RwLock<Snapshot>>` background-task idioms
(`self_metrics`, `telemetry::sampler`); naively reusing that shape here (a
`tokio::spawn` loop calling into rusqlite directly, or `spawn_blocking` per
call) either blocks an async worker thread on every query or pays a thread-pool
hop for every single insert — real cost at the ~1 write/sec cadence the
sampler drives once persistence is wired in.

SQLite itself also only safely supports one writer at a time; WAL mode lets
readers proceed concurrently with a writer, but two writers still serialize
against each other at the database level. A pool of writer connections would
just add contention and complexity for a lock that already collapses to "one
writer" underneath.

## Decision

**One dedicated `std::thread` — not `tokio::spawn` — owns the single write
connection for the service's entire lifetime**, fed by a bounded
`tokio::sync::mpsc::Receiver<WriteCommand>`. The thread loop uses
`rx.blocking_recv()`, which is the correct/expected way to bridge a
synchronous resource into an async caller (the inverse of `spawn_blocking`:
here the blocking work has a long-lived home instead of hopping to the
blocking thread pool per call).

```rust
pub enum WriteCommand {
    InsertTelemetrySnapshot(Box<TelemetrySnapshotRow>),   // fire-and-forget
    InsertAuditEvent { new: NewAuditEvent, reply: oneshot::Sender<Result<AuditEventRecorded, RepositoryError>> },
    RunRetentionSweep { now: DateTime<Utc>, reply: oneshot::Sender<Result<RetentionReport, RepositoryError>> },
    Shutdown { reply: oneshot::Sender<()> },
}
```

- `InsertTelemetrySnapshot` is fire-and-forget: the sampler's tick loop must
  never block on persistence, matching the "slow consumer must not block
  sampling" rule already applied to `self_metrics`/`sampler`. `try_send` is
  used for the production path (`try_persist_telemetry_snapshot`); a full
  channel or absent repository drops the sample (`tracing::debug!`), never
  blocks. Tests that need every row to land use the awaited `.send()` path
  directly (bypassing the fire-and-forget wrapper) instead.
- `InsertAuditEvent`/`RunRetentionSweep` carry a `oneshot` reply — low
  frequency, callers (or tests) may need the result back deterministically.
- **No writer pool, ever.** Pooling writers doesn't parallelize anything —
  SQLite still serializes them internally — it only adds queueing and lock
  contention for a resource that's already effectively single-threaded.
- A small `r2d2`/`r2d2_sqlite` **read-only** pool (size 4) is separate and
  safe under WAL: readers never block the writer, and vice versa.

**Audit hash-chain state lives only as local variables inside the writer
thread.** `next_audit_id` and `last_row_hash` are seeded once at startup
(`SELECT ... ORDER BY id DESC LIMIT 1`, or the genesis constant if empty) and
then updated in-thread after each insert. Because there is exactly one
writer, this read-then-write is trivially race-free — no DB-side locking,
no `SELECT ... FOR UPDATE` equivalent, needed. The writer computes each row's
`row_hash` using the next id *before* inserting, then does an explicit-id
`INSERT INTO audit_events (id, ...) VALUES (?1, ...)` — rows can't be
silently reordered without invalidating the chain, since the id is baked
into the hash rather than assigned by `AUTOINCREMENT` after the fact.

**Migrations are up-only, deliberately.** Refinery supports down migrations
only via paired `U*__name.sql` files; none are provided. A "down" migration
for `audit_events` is nonsensical (it can't un-invalidate a hash chain the
data-owning system may already be relying on), and forward-only avoids a
whole class of corruption-via-broken-rollback. `repository::init`'s startup
sequence — open connection, set pragmas, run migrations inside refinery's own
transaction, seed chain state, open the read pool, spawn the writer — treats
a migration failure as fatal to the repository layer only: it returns `Err`,
`main.rs` logs it and sets `state.repository = None`, and the service keeps
serving live (non-persisted) telemetry. The database file itself is never
dropped or recreated on failure.

## Consequences

- Every `core::repository` write funnels through one channel and one thread;
  there is no risk of two threads racing on the same `rusqlite::Connection`
  (rusqlite's `Connection` is `!Sync`, so this isn't just a design
  preference — the alternative would need a `Mutex<Connection>` wrapping the
  same serialization anyway, with worse ergonomics).
- Startup must create the database file's parent directory before opening
  it. This was missed in the first implementation — `open_write_connection`
  called `Connection::open` directly against
  `$XDG_DATA_HOME/ai-ops-coordinator/core.sqlite3` without first creating
  `ai-ops-coordinator/`, which does not exist on a first-ever run. A live
  E2E run against a fresh temp `$XDG_DATA_HOME` caught this immediately (the
  service silently degraded to `repository: None`, exactly per its
  fail-open contract, but for the wrong reason — every real first-time user
  would hit this). Fixed by `create_dir_all`-ing the parent directory before
  `Connection::open`, mirroring the pattern `transport::unix::serve` already
  uses for the socket's parent directory.
- Two other real bugs surfaced only by live execution against the running
  binary, not by the unit/integration test suite, and are recorded here as
  the reason this ADR treats "start the real service and hit it" as
  load-bearing verification, not optional polish:
  - The audit-chain row hash originally hashed `row.ts`'s full nanosecond
    precision, but the DB persists/reads `ts` at millisecond precision —
    verify-time recomputation never matched the insert-time hash for *any*
    row. Fixed by hashing the same millisecond-precision string that is
    actually persisted.
  - The history endpoints' `range` query parameter used
    `#[serde(rename_all = "snake_case")]` on variants like `Last24h`, which
    serde renders as `last24h` (no underscore before a digit) rather than
    the `last_24h` the API's own documented contract specifies — `#[serde
    (rename_all = "snake_case")]`'s word-boundary detection only fires on
    letter case transitions, not letter-to-digit ones. Fixed with explicit
    `#[serde(rename = "...")]` on the three digit-containing variants, and
    locked in with a regression test
    (`digit_containing_range_params_use_underscored_names`).
- Cross-target CI (`check-windows`, `check-macos`) had to move from
  cross-compiling on `ubuntu-latest` to running natively on `windows-latest`/
  `macos-latest`. `rusqlite`'s `bundled` feature compiles SQLite's C
  amalgamation via the `cc` crate in a build script, and that build script
  runs even for `cargo check` (Cargo can't skip a dependency's build script
  just because the top-level command is `check`); `ubuntu-latest`'s `cc` has
  no cross toolchain for the MSVC or macOS ABI, so the C compile itself
  fails before any of our own code is even reached. Confirmed locally by
  reproducing the exact ubuntu-latest + `rustup target add` setup the old CI
  job used. Native runners sidestep the cross-toolchain problem entirely and
  are the standard fix once a C dependency enters the graph; the tradeoff is
  GitHub Actions' higher per-minute cost for `macos-latest` runners.
- A future unit that wants a writer *pool* (e.g. if profiling ever shows the
  single writer thread as an actual bottleneck) must write a new ADR
  justifying it, per the "later unit deviates via a new ADR" convention —
  this isn't expected to happen, since SQLite's own single-writer semantics
  mean a pool can't parallelize writes regardless of how many threads offer
  them.
