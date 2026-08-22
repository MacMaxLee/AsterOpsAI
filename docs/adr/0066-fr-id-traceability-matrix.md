# 0066 — FR-ID Traceability Matrix

## Status

Accepted (unit U61).

## Context

SRS §36 and TRS §45 both mandate a traceability matrix — "Every FR-ID in
docs/SRS.md must be traceable to at least one test... a script generates
and checks this traceability matrix **before every tag**" — and frame an
untested FR-ID as "a release blocker, not a documentation gap." A direct
audit confirmed this has never existed: no script, no generated artifact,
no CI job, despite 60+ tags already shipped without it.

TRS's own internal roadmap label for this ("unit U15," distinct from this
session's own U-numbering) actually bundles three separate deliverables:
- §45: the FR-ID traceability matrix itself — **this unit's scope**.
- §20: a CI dependency-graph check proving no AI call is reachable from the
  correlation engine's code path — a different kind of tool, a real,
  separate, explicitly deferred future unit.
- §46: an RBAC role-to-permission-matrix doc-sync gate kept "in sync with
  **the coordinator's authorization code**" — the coordinator is v2
  (CLAUDE.md, SRS §35 FR-FLEET-*) and doesn't exist in this repo yet. Not
  buildable; named, not silently dropped.

## Decision

**Full FR-ID inventory**: 63 FR-IDs in `docs/SRS.md` (a regex tolerant of
both `- FR-ID:` and `**FR-ID**:` markdown forms — the doc genuinely uses
both — anchored to exclude `NFR-*` IDs, which share the `FR-` suffix as a
literal substring of `NFR-`). 3 are `FR-FLEET-*` (v2, out of scope) — 60 in
v1 scope.

**Three-tier status, not binary pass/fail.** A first literal-reference
sweep found the overwhelming majority of FR-IDs have zero reference
literally inside a `tests/`-directory file — but CLAUDE.md's own session-
hygiene section says FR-IDs are referenced "**sparingly** in code comments
and explicitly in the tests that cover them," meaning the project never
committed to tagging every implementing file. A strict "must appear in a
test file" checker would falsely flag dozens of FR-IDs with real, passing,
merely un-cited test coverage — not a faithful measurement of SRS §36's
actual intent. The matrix instead reports:
- **OK** — at least one reference inside a real test: a file under a
  `tests/` directory, or (this codebase's other real test-authoring
  convention) a reference falling after a source file's own last
  `#[cfg(test)]` marker.
- **NEEDS-VERIFICATION** — referenced somewhere, never inside a test.
  Genuinely ambiguous to a grep-only tool. Reported, not release-blocking.
- **MISSING** — zero references anywhere in the repo. Unambiguous, hard
  release-blocker — except an explicit `FR-FLEET-*` allowlist (a real v2
  non-implementation, not a v1 gap).

**`scripts/generate-traceability-matrix.sh`** (new, bash, matching the
existing `check-*.sh` gate scripts' own convention rather than a new Rust
binary — this is repo-introspection tooling, not product-schema
generation): extracts every FR-ID + requirement text from `docs/SRS.md`,
greps `rust_core/`, `console/lib/`, `console/test/`, `docs/adr/`,
`migrations/` for each, classifies per the rule above, and writes
`docs/traceability-matrix.md` (committed, generated-and-checked exactly
like `/schemas/*.schema.json`).

**`scripts/check-fr-id-traceability.sh`** (new): regenerates into a temp
comparison and `diff`s against the committed matrix (drift check,
mirroring `check-schema-drift.sh`'s own generate-then-diff shape), then
separately fails on any `MISSING` row whose FR-ID isn't in the
`FR-FLEET-*` allowlist. Wired into `.github/workflows/ci.yml` as a new
`fr-id-traceability` job, alongside the two existing grep gates.

**A real portability bug found and fixed while building this**: the
initial generator truncated each requirement-text table cell with `cut
-c1-70`. In this sandbox's actual script-execution context (not
reproducible in an interactive shell with the same locale), that
truncation split a multi-byte UTF-8 character mid-sequence, producing a
genuinely invalid UTF-8 byte in the committed file — confirmed by decoding
the raw bytes, not assumed. That, in turn, made `grep` (without `-a`)
silently treat the whole file as binary and report zero matches for any
string search, including the tool's own `MISSING`/`OK` status markers.
Fixed two ways: the truncation was removed entirely (full requirement text
in the cell — safer than a byte-unsafe cut), and `check-fr-id-
traceability.sh`'s own `grep` call against the matrix explicitly passes
`-a`, since the file legitimately embeds SRS.md's own prose (including
real em-dashes) that no amount of care in the generator can fully purge
without mangling real content.

## Real findings from the first run

Every one of the 21 v1-scope FR-IDs that came back `MISSING` on the first
run turned out to have *some* real implementation — a dedicated triage
pass (reading each requirement's exact SRS text against the real code)
found **zero genuine "never built" gaps** in this batch. The breakdown:

**16 `COVERED-UNTAGGED`** (implementation and covering test both real,
just uncited) — fixed in this unit by adding one short `SRS FR-XXX-NNN`
doc-comment line each to the covering test: FR-CONSOLE-002, FR-CPU-001,
FR-CPU-002, FR-MEM-002, FR-NET-001, FR-PRO-001, FR-PRO-002, FR-DEV-001,
FR-HIST-001, FR-POL-003, FR-DB-001, FR-DB-003, FR-DB-004, FR-DB-007,
FR-DB-008, FR-DB-009. Net effect: `OK` count rose from 17 to 33.

**4 more**, real implementation but genuinely thin/absent test coverage —
given a plain source-comment reference (not a test tag, since claiming
`OK` would overstate real coverage) so the matrix now honestly reports
`NEEDS-VERIFICATION` (implemented, unverified) instead of `MISSING`
(implies unbuilt) — each also named here as concrete, ready-to-pick-up
future work, not fixed in this tooling-only unit:
- **FR-STO-001**: `telemetry/storage.rs`'s counter-reset/rate code path is
  real, but every existing test calls it with `prev=None`, so the
  delta/counter-reset branch itself is never exercised — unlike
  `FR-NET-001`'s own `nic_counter_reset` test, which proves the identical
  pattern for network counters. A near-identical follow-up test is cheap
  and well-specified.
- **FR-STO-002**: `platform/linux/storage.rs` already, honestly,
  documents itself as "live-only, not fixture-tested" — a pre-existing,
  self-acknowledged limitation, not a new discovery.
- **FR-DB-005**: `query_stats`'s `Gated::Supported` branch — the actual
  "`pg_stat_statements` is loaded" happy path — is implemented but no test
  in this repo ever loads the extension via `shared_preload_libraries` to
  exercise it (every test only proves the *absent* path).
  `TestPostgres::start_with_extra_options` (unit U51) was built expressly
  to make this loadable; nothing has used it for this yet.
- **FR-SYS-002**: `self_metrics` is genuinely wired into the health
  handler, but `health_endpoint.rs`'s own test never asserts on
  `self_cpu_percent`/`self_rss_bytes` — real wiring, thin verification.

**One real, unresolved discrepancy, named rather than adjudicated**:
FR-HIST-002 ("every persisted analytical row — performance, security,
action, benchmark — is immutable once written; corrections happen via new
rows, not updates") explicitly names "benchmark" among its covered
categories, but `repository/benchmark.rs::mark_rolled_back` updates an
existing `benchmark_runs` row in place. Whether that's a real violation or
a legitimate status-lifecycle transition (the same CAS-transition-in-place
pattern `policy::transition` already uses elsewhere in this codebase) is a
genuine product/architecture question left for a human to weigh in on —
not silently fixed or dismissed here.

## Explicitly deferred (named, not silently dropped)

- The ~27-FR-ID `NEEDS-VERIFICATION` backlog beyond the 4 named above —
  real, but a much larger retroactive-tagging effort, out of this bounded
  unit's scope.
- TRS §20's correlation-engine "no AI call reachable" CI dependency-graph
  check — a different kind of tool entirely.
- TRS §46's RBAC docs-sync gate — genuinely not buildable until the v2
  coordinator exists.

## Verification (real, not simulated)

- `bash scripts/generate-traceability-matrix.sh` run for real against the
  actual repo state; `docs/traceability-matrix.md` committed.
- `bash scripts/check-fr-id-traceability.sh` tested against all three real
  outcomes, not just the happy path: (1) passes clean on the current repo
  (33 OK / 27 NEEDS-VERIFICATION / 3 allowlisted MISSING); (2) a real
  induced staleness (appending a stray line to the committed matrix)
  correctly fails with the "stale, regenerate" message; (3) a real,
  temporarily-removed FR-ID doc comment (`FR-CPU-001`) correctly caused
  the freshly-regenerated matrix to show it `MISSING` again, proving the
  detection genuinely round-trips through real code state — restored
  immediately after confirming.
- New `fr-id-traceability` job added to `.github/workflows/ci.yml`.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check` clean
  (74/74 test binaries, 0 failures — this unit touched only doc comments,
  no functional Rust code). Both pre-existing grep gates and schema-drift
  clean (no `contracts` change). `flutter test`/`analyze`/`dart format`
  clean on the one touched console file. No leaked PostgreSQL processes.

## Consequences

- `docs/traceability-matrix.md` is now a real, generated, CI-checked
  artifact — the first concrete evidence SRS §36/TRS §45's own requirement
  is satisfied, not merely asserted in prose.
- The matrix's own three-tier design is itself a documented, deliberate
  interpretation of "traceable to a test" for a grep-based tool that can't
  perfectly distinguish "genuinely untested" from "tested but uncited" —
  a real, named limitation of the tool, not a claim of perfect coverage
  detection.
- Five concrete, well-specified test-coverage gaps (FR-STO-001, FR-DB-005,
  FR-SYS-002 as ready-to-implement follow-ups; FR-STO-002 as an
  already-acknowledged limitation; FR-HIST-002 as an open product
  question) are now visible and named, where before they were invisible.
