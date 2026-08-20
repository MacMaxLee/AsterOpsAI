# 0010 — Performance Analysis: Thresholds, Known Gaps, and a `DatabaseInfo` Scope Amendment

## Status

Accepted (unit U5).

## Context

U5 implements SRS §11's FR-PERF-001..005: deterministic host bottleneck
classification, database health classification when PostgreSQL is
monitored, two frozen 0-100 scores, and a hard no-AI boundary. Several
concrete questions came up that neither SRS/TRS pins nor any earlier unit
resolved, each settled by reading the actual code rather than guessed —
recorded here so the reasoning survives past this session, matching ADR
0006's own precedent for judgment-call thresholds.

## Decision

**New deterministic thresholds** (`core::analysis::thresholds`) for
STORAGE_IO (`io_latency_ms`), NETWORK (combined error+drop rate as a
fraction of packet rate — link-speed-independent since no interface
bandwidth is known to this codebase), and all eleven DB health categories
are judgment calls, exactly like ADR 0006's CPU/memory tables: named
constants directly above the classifying functions, boundary semantics
pinned by tests (`<` for NORMAL/ELEVATED/HIGH, `>=` for CRITICAL). A future
unit that wants to tune these does so via a new ADR, not a silent constant
edit.

**Host evidence is a window of already-persisted `telemetry_snapshots`
rows** (`repository::query_recent_snapshots`, new this unit), not a single
sample: a domain (CPU/MEMORY/STORAGE_IO/NETWORK) only "crosses" when at
least `MIN_SAMPLES_FOR_CLASSIFICATION` (3) samples are available and at
least `SUSTAINED_FRACTION` (60%) of them are High/Critical tier —
sustained-signal, not single-sample noise, matching the spirit of
FR-SYS-003's existing sampler back-off. Zero samples in the window produces
`HostBottleneck::Unknown` with an explicit "no data" evidence item, never a
silent `NONE` (FR-PERF-002 requires a verdict without evidence to be
invalid; claiming NONE from nothing would violate that in spirit).

**THERMAL and POWER are real, reachable `HostBottleneck` variants that the
classifier can never actually produce today — a flagged, permanent-for-now
gap, not a fabricated sensor reading.** `grep -rniE
"thermal|temp|battery|power|watt|sensor"` across `rust_core` (checked
before writing any code) confirms zero data source anywhere: not in
`PlatformAdapter` (linux/macos/windows), not in any `contracts::telemetry`
snapshot type. SRS pins the exact ten-value enum, so the variants stay;
`analysis_host_classification_test.rs`'s
`thermal_and_power_are_never_produced_by_any_fixture_in_this_suite` test
asserts the gap directly rather than leaving it implicit. Closing it is a
future platform-adapter unit's job — adding real sensor reads, not an
`analysis`-layer change.

**BACKGROUND classification needs live per-process data the stored
telemetry window doesn't carry.** `telemetry_snapshots` stores
`process_total_count` (a count) but no per-process breakdown, so a crossed
CPU/MEMORY domain can only be reattributed to BACKGROUND when the caller
optionally supplies a current `Vec<ProcessInfo>` alongside the historical
window (`classify_host`'s `processes: Option<&[ProcessInfo]>` parameter).
When BackgroundService-categorized processes account for at least
`BACKGROUND_MAJORITY_FRACTION` (50%) of the crossed domain's live
cpu_percent/rss_bytes, the verdict becomes BACKGROUND instead of CPU/MEMORY,
with the specific PIDs in evidence. When `processes` is `None`, this
refinement is skipped entirely — flagged, not guessed — proven by
`without_process_data_background_is_never_produced`.

**DB evidence is a single live poll, not a window — `Deadlocks` and
`TempFileUsage` are evaluated as absolute counts since `stats_reset`, not
rates.** U4's own design note already established `core::dbms`'s 13
methods as stateless snapshots ("rate calculation belongs to whichever
later unit wires up a live poll loop"), and `dbms_snapshots` is
schema-only with nothing writing to it yet. `classify_db` is accordingly
pure and synchronous over a caller-assembled `DbEvidenceBundle` — one round
of live `DbmsAdapter` calls, never an adapter reference or async code
inside `core::analysis` itself. This is a real, honest limitation (a
`Deadlocks` count of 3 means nothing about *when* those deadlocks
happened) that closes automatically once a later unit adds the live poll
loop with persisted `dbms_snapshots` history to diff against.

**SCOPE amendment: `DatabaseInfo` gains `xact_commit`/`xact_rollback`.**
FR-PERF-003 explicitly requires rollback-ratio classification, and no
existing `core::dbms` type carries commit/rollback counters — `DatabaseInfo`
previously had only `name`/`size_bytes`. `list_databases()`'s query now
left-joins `pg_stat_database` for the two counters (`COALESCE`d to 0 for
any database with no stats row yet). Flagged here rather than silently
expanded, matching U2/U3/U4's own "flag the amendment, don't silently
expand scope" precedent. No migration needed: `DatabaseInfo` isn't
persisted as SQL columns (`dbms_snapshots` stores JSON blobs, unwritten
either way, same as every other `core::dbms` type).

**Availability is evaluated by the caller, not `classify_db`.** A poll that
fails outright (connection refused, timeout) never produces a
`DbEvidenceBundle` to classify — conflating "the poll failed" with "the
poll succeeded and found problems" inside one function would blur two
different failure domains. `classify_db` always reports `Availability: Ok`
(it only ever runs after a successful poll); `unavailable_verdict(reason,
now)` is the separate function a caller uses when the poll itself errors,
marking `Availability: Critical` and every other category `Unavailable`.

**Two frozen weight tables** (`core::analysis::score`): `HOST_SCORE_VERSION
= "host-v1"`, `DB_SCORE_VERSION = "db-v1"`. Both are pure functions —
`score_host` sums a fixed per-domain-tier penalty (5/15/30 for
Elevated/High/Critical) subtracted from 100, floored at 0, skipping domains
with no data; `score_db` sums a fixed per-category-status penalty (10/25
for Warning/Critical) the same way, except an `Availability: Critical`
poll-failure short-circuits the whole score to 0 (every other category is
`Unavailable` in that case anyway, so summing zero penalties would
misleadingly imply a clean bill of health). Per SRS FR-PERF-004, a future
retuning introduces a new `*_SCORE_VERSION` constant; it never mutates
`host-v1`/`db-v1` in place, and historical `performance_analysis` rows are
never re-scored retroactively.

**No AI dependency anywhere in `core::analysis` (FR-PERF-005).** The module
has zero imports beyond `chrono`, `serde_json` (deserializing the JSON blob
columns), and this crate's own `contracts`/`dbms`/`repository` types — no
HTTP client, no AI-provider type. A later unit (U15, per TRS §20) adds a
CI dependency-graph check proving the correlation engine can't reach an AI
call either; this unit doesn't need that check to already exist to be
correct, since the module's dependency list is a fact checkable by reading
`analysis/*.rs`'s own `use` statements today.

## Consequences

- THERMAL/POWER stay permanently unreachable until a platform-adapter unit
  adds real sensor exposure — anyone consuming `HostBottleneck` should not
  assume these two variants are dead code to be removed; they're load-bearing
  for SRS compliance even while unused.
- BACKGROUND classification quality depends entirely on whether the caller
  bothers to supply live process data — the live poll loop wiring unit
  should pass the current `ProcessSnapshot.processes` through, not skip it
  for convenience.
- DB `Deadlocks`/`TempFileUsage` tiers will read as more "startup-noise
  sensitive" than genuinely rate-based metrics until a live poll loop with
  persisted `dbms_snapshots` history lets a later revision diff two polls
  instead of reading one absolute counter.
- `analysis_db_health_live_test.rs` proves the `DbEvidenceBundle` assembly
  compiles and behaves against real `DbmsAdapter` output types (not just
  hand-built fixtures), reusing `core/tests/dbms/common/mod.rs`'s harness —
  no new test infrastructure was needed this unit.
