# 0057 — Test Harness: Real Streaming-Replication Standby Coverage

## Status

Accepted (unit U52).

## Context

ADR 0042 (unit U37) named the last remaining test-coverage gap from
this session's earlier project-wide gap audit: `replication_status`'s
standby-populated path — a non-empty `standbys` list on the primary,
and the in-recovery response shape on the standby's own side — had
never been proven against a real PostgreSQL instance, since
`support::TestPostgres` had no standby-pair capability. ADR 0042
explicitly called this "a real, separate, much larger future
capability" than the `pg_stat_statements` gap U51 (ADR 0038/0056) just
closed, and that framing held up under direct research: closing it
needed a second full instance (new harness code, not a one-line start
flag) plus a genuinely asynchronous, previously-unproven wait
condition — `pg_stat_replication` catch-up — not just a start-time GUC.

Confirmed tractable before implementing: `pg_basebackup`/
`pg_receivewal` are both present in this sandbox's installed PG17
`bin/` directory, and `core::dbms::adapters::postgresql::queries::
replication_status`'s two branches (`pg_is_in_recovery()` true → the
standby's own view; false → the primary's `pg_stat_replication`-backed
view) were already correct, unmodified `core` code — this unit closes
a pure test-harness gap, not a product fix.

## Decision

**`support::TestPostgres`**: refactored the shared "run `pg_ctl
start`, wait for real readiness, construct `Self`" tail (previously
inlined in `start_with_extra_options`, U51) into a new private
`start_pg_ctl` helper. Added `start_standby_of(&self, major_version)`:
builds a standby via a real `pg_basebackup -R` against `self`'s own
socket, then reuses `start_pg_ctl` to bring it up. `-R` auto-writes
`standby.signal` + `primary_conninfo`; the default
`--wal-method=stream` (PG10+) needs no extra flags. Streaming
replication works entirely over the same Unix-domain socket every
instance already uses — `primary_conninfo`'s `host` accepts a socket
directory, and `initdb --auth=trust`'s own default `pg_hba.conf`
already trust-authenticates `replication` connections, so no TCP
listener or extra auth setup was needed. `start`/`start_with_extra_
options`'s public signatures and behavior are completely unchanged —
verified by the full pre-existing test suite passing unmodified.

**`service/tests/dbms_endpoints.rs`**: new
`replication_status_reports_a_real_standby_once_streaming` — starts a
primary, calls `start_standby_of` to build a real standby, polls the
primary's real `pg_stat_replication` until the standby genuinely
registers (the same "poll real state, never a guessed sleep"
discipline this project has used throughout), then proves both real
HTTP views: the primary reports exactly one real streaming standby;
the standby reports its own in-recovery state with an empty `standbys`
list — the branch `replication_status_reports_a_real_primary_state`
could never exercise.

## Verification (real, not simulated)

- New test passed cleanly on the very first real attempt and 3/3
  reruns in isolation — genuine streaming replication set up,
  registered, and proven over HTTP for both the primary and standby
  views, no flakiness observed in this new code path.
- Full `dbms_endpoints.rs` suite (24 tests, 1 new) and full workspace
  `cargo test --workspace --test-threads=1` both clean. One
  pre-existing, already-documented flaky test observed during the full
  run (`deadlock_history_reports_a_real_induced_deadlock`, a real
  50ms-timing-sensitive race, file untouched by this unit) — reran
  clean 3/3 in isolation, confirmed pre-existing and unrelated.
- `cargo build/clippy -D warnings/fmt --check` clean. Both grep gates
  clean (new `Command`/SQL usage is inside `service/tests/support/
  mod.rs`/`dbms_endpoints.rs`, already allowed locations). Schema-drift
  clean (no `contracts` change). `cargo deny check`/`cargo audit`
  clean. No leaked PostgreSQL processes (both primary and standby
  explicitly stopped).

## Consequences

- `replication_status`'s both branches — primary-with-standbys and
  standby-in-recovery — now have real, end-to-end HTTP coverage. This
  closes the last remaining item from this session's earlier
  project-wide gap audit of still-open ADR-named gaps.
- `TestPostgres::start_standby_of` is a real, reusable harness
  capability — any future test needing a genuine replication topology
  (e.g. a future console replication-lag demo scenario) can build on
  it directly, not scoped narrowly to this one test.
- No console surface for replication exists yet — unrelated, separate,
  later work, matching ADR 0042's own consequences section.
