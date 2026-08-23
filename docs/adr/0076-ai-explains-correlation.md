# 0076 — AI-Explains-Correlation: `build_host_bundle`/`build_db_bundle`'s Third Sibling

## Status

Accepted (unit U70).

## Context

ADR 0072 found FR-CORR-003's console half real and wired, but named the
remaining gap explicitly: no AI-explanation feature existed for
correlation output, unlike host and DB analysis (`ai::bundle::
build_host_bundle`/`build_db_bundle`, wired since units U45/U47). This
unit builds it — confirmed with the user first, given it's a real
feature addition (new bundle builder, new endpoint, new console UI), not
a test or doc fix like the rest of this session's backlog work.

## Decision

**`core::ai::bundle::build_correlation_bundle(result: &CorrelationResult,
subject: &str) -> EvidenceBundle`** (new): mirrors `build_host_bundle`/
`build_db_bundle` exactly. Only `ranked` hypotheses contribute evidence
and candidates — `ruled_out` carries a reason string, not `Evidence`
items, and an explanation is for naming which *live* hypothesis the
evidence supports, not restating what's already dismissed. Candidates
are the ranked hypotheses themselves (`RootCause::as_str()` + confidence),
not an incidental entity like `host_candidates`'s processes or
`db_candidates`'s sessions/tables — the AI needs to name *which cause*
precisely, using the same literal label the wire type already
serializes. `verdict_label` is the top-ranked cause, or `"NONE"` if
every cause was ruled out (mirrors `HostBottleneck::None`'s own
"nothing to explain" semantics). This adds a new `ai::bundle` →
`correlation` dependency edge — the opposite direction from the one
`check-no-ai-reachable-from-analysis-or-correlation.sh` (ADR 0068/0069)
forbids, confirmed directly by re-running that gate rather than assumed
safe.

**`service::api::v1::analysis::explain_correlation`** (new): identical
shape to `explain_host`/`explain_db` — checks for a configured AI
provider first, recomputes `correlate()` fresh (same "no caching a stale
verdict across requests" precedent every explain endpoint follows,
rather than reusing the `correlation` handler's own result), builds the
bundle, and routes through the same `try_explain` degrade contract (SRS
FR-AI-001: absent/unreachable/timeout/garbage/unresolved-citation all
collapse to one honest `Unavailable`). New route:
`/api/v1/analysis/correlation/explain`.

**Console**: `correlation_screen.dart` has a real, documented layout
constraint from unit U21 — ranked hypotheses, evidence, and the
ruled-out list must all be visible with no scrolling or toggle (the
demo's own REQUIREMENTS #5). Rather than reopening that exact overflow
risk with an unbounded inline explanation section (`host_analysis_
screen.dart`'s own pattern, which works there because that screen is
already a scrolling `ListView`), the explain button opens a **dialog**
instead — the fixed layout underneath is completely untouched; the
dialog scrolls internally if the explanation is long. The dialog's
content (`_ExplanationSection`/`_GatedExplanation`/`_GatedMessage`/
`_ExplanationContent`) otherwise duplicates `host_analysis_screen.dart`'s
own shape verbatim — matching this codebase's own established
convention of duplicating this pattern per-screen (`database_screen.
dart` already does the same, citing ADR 0050) rather than extracting a
shared widget three screens deep into an already-repeated pattern.

**A real Flutter bug found while building this**: `AppLocalizations.of
(context)` can't be called from `initState()` — inherited-widget lookups
aren't safe until dependencies resolve. The dialog's "fire the request
immediately on open" behavior (a deliberate UX choice, since the user
already tapped "Explain with AI" once to open the dialog — a second tap
inside would be redundant) needed `didChangeDependencies()` instead,
guarded with a `_started` flag so a later dependency change (e.g. a
locale change) doesn't refire it.

**A real test-authoring bug found while writing widget tests**:
`FakeTransport` matches by path *prefix*, first-inserted-key-wins, and
`/analysis/correlation/explain` also starts with `/analysis/correlation`
— queuing the base path before the explain path caused the explain
request to be wrongly satisfied by the (already-consumed, now empty)
base-path queue. `host_analysis_screen_test.dart` already established
the fix as a real convention (queue the more specific path first) that
this unit's own tests hadn't followed initially.

## Verification (real, not simulated)

- `build_correlation_bundle`: exercised for real (not just unit-tested
  in isolation) via the full HTTP round trip in the new service tests.
- New `service/tests/ai_endpoints.rs` tests, mirroring `explain_host`'s
  own three: no provider → `503`; provider configured but genuinely
  unreachable (a real closed TCP port) → `200 OK` +
  `GatedValue::Unavailable`; a real one-shot TCP server standing in for
  Ollama → `200 OK` + `GatedValue::Supported` with the real parsed
  explanation. All 9 tests in the file (3 host + 3 db + 3 correlation)
  pass together.
- New `console/test/correlation_screen_test.dart` tests: the same three
  states, through a real dialog open/tap/pump cycle, plus a real retry
  after a transport-level error. All 6 tests in the file (3 existing +
  3 new) pass together, including the existing REQUIREMENTS #5
  no-scrolling test — confirming the dialog doesn't touch that
  constraint.
- `scripts/check-no-ai-reachable-from-analysis-or-correlation.sh`
  re-run and still passes — the new `ai::bundle` → `correlation` edge is
  the opposite direction from what that gate forbids.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean. `scripts/check-schema-drift.sh` and `scripts/check-console-
  codegen-drift.sh` both clean — no new wire types were needed (`AiExplanation`/
  `GatedValue` were already shared).
- `core::correlation::mod.rs`'s own FR-CORR-003 doc comment (already
  corrected once in ADR 0072 to stop claiming "nothing wired") is
  updated again to state the now-fully-real status, with a citation
  added to the new end-to-end test so the traceability generator
  reflects it honestly.

## Consequences

- `docs/traceability-matrix.md`: **60 OK, 3 NEEDS-VERIFICATION, 0
  MISSING** (of 63 total FR-IDs) — the only remaining
  `NEEDS-VERIFICATION` rows are the three `FR-FLEET-*` v2 IDs (ADR
  0071's design proposal, not yet implemented by choice).
- A future correlation-output consumer (a fleet-coordinator alert rule,
  say) has a real precedent for both its wire representation
  (`contracts::correlation`) and its AI-explanation path
  (`build_correlation_bundle`) to build on, rather than inventing either
  from scratch.
