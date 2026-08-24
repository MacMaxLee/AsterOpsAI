# 0084 — cgroup v2 Swap Accounting, Plus Two Stale-Doc Corrections

## Status

Accepted (unit U79).

## Context

A wider second-pass scan (beyond ADRs, into source doc comments
directly) found three real items:

1. `telemetry::memory`'s own doc comment named a real, accepted
   simplification: *"always the host's swap, even when containerized.
   cgroup v2 has its own per-container swap accounting
   (memory.swap.current/memory.swap.max), but reading it is out of
   scope for this unit."* A containerized deployment's memory-pressure
   classification was mixing cgroup-scoped memory with host-scoped
   swap — a real accuracy gap, not cosmetic.
2. `dbms::mod.rs`'s module doc still described the DBMS layer as "not
   yet wired into `service`'s HTTP surface, a background poll loop, or
   the console" — false since unit U20 (HTTP surface), U72/ADR 0078
   (independent poll loop), and the console's `database_screen.dart`
   all shipped since that text was written. Confirmed stale by direct
   grep, not assumed.
3. `tuning::mod.rs`'s doc similarly claimed `AskBeforeChanges`'s
   human-facing "ask" was "a future unit's job" — false since
   `process_candidate` already routes through the same `policy::
   evaluate` every action type uses, landing in the same generic
   policy inbox (`core/tests/tuning_plan_recommend_and_ask_test.rs`'s
   own `ask_before_changes_proposes_for_real_but_never_authorizes`
   proves it). No tuning-specific console work was ever needed, since
   the inbox was already generic.

## Decision

**(1) Real fix, not just a doc correction.** `telemetry::memory::
cgroup_memory` now also reads `memory.swap.max`/`memory.swap.current`
from the same cgroup path it already resolves for `memory.max`/
`memory.current`, via a new `cgroup_swap` helper mirroring its exact
shape (including the same `"max"`-means-unbounded handling). When
present and parseable, the cgroup's own swap figures — not
`/proc/meminfo`'s host-wide `SwapTotal`/`SwapFree` — are what
`MemorySnapshot` reports and what `classify_memory_pressure` sees.
When absent (swap accounting disabled, or a cgroup v1 host), the
exact previous host-wide fallback behavior is preserved — no new
failure mode, only a real improvement when the data is actually
available.

**(2) and (3): stale doc comments corrected in place**, each rewritten
to describe current, verified reality with a citation to the unit/ADR
that actually closed the gap — the same "keep doc comments honest"
practice this session applied repeatedly (ADR 0072's FR-CORR-003
correction, ADR 0077/0078's `gucs.rs` correction).

## Verification (real, not simulated)

- `telemetry::memory` (2 new/extended tests): `container_reports_
  cgroup_limit`'s existing fixture (no swap files) proves the fallback
  path is unchanged (snapshot unchanged, confirmed byte-for-byte via
  `insta`); a new `tests/fixtures/proc/container-swap-accounted/`
  fixture (real `memory.swap.max`/`memory.swap.current` files, values
  deliberately different from the fixture's own host-wide `SwapTotal`/
  `SwapFree`) proves the cgroup-scoped values are genuinely what gets
  reported, not silently ignored.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. Both grep gates and the AI-reachability gate pass
  (unaffected).
- `docs/traceability-matrix.md` regenerated: no FR-ID drift.

## Consequences

- A containerized deployment's memory-pressure classification is now
  accurate to what's actually enforced against it (cgroup-scoped swap
  ceiling and usage), not a host-wide figure the container may not
  even be able to reach.
- Two more doc comments stopped actively misleading a future reader
  about what's built — the recurring lesson this session keeps
  re-learning: a "not yet" comment has a shelf life and needs revisiting
  once the unit it names actually ships.
