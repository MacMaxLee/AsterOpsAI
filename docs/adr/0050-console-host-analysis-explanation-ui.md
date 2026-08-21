# 0050 — Console: Host Analysis AI Explanation UI

## Status

Accepted (unit U45).

## Context

U44 shipped `GET /api/v1/analysis/host/explain` (ADR 0049) but stayed
service-only, explicitly naming the console side as future work. This
unit closes that gap.

## Decision

**Deliberately on-demand, not polled.** Every other console provider
in this project is `PolledRepository` + `StreamProvider.autoDispose`,
re-fetching every 1-10s per `refreshIntervalProvider`. This endpoint is
different: a real AI inference round-trip is genuinely expensive
(`AiProviderConfig`'s own default timeout is 10s), so polling it on
the same cadence as a CPU snapshot would hammer the AI provider for no
reason. ADR 0034 (unit U29) already established the exact right
precedent for this situation — `/actions/resumable`'s own "on-demand
lookup, not background polling" design, because *"polling in the
background for every visible row would be a real, avoidable cost."*
This unit applies that identical reasoning: `_ExplanationSection`
(a `ConsumerStatefulWidget` on `host_analysis_screen.dart`, mirroring
`processes_screen.dart`'s own `_ResumeProcessDialogState` shape — a
simple stage enum, `ref.read(apiClientProvider)`, not `ref.watch` a
stream) fires the request exactly once per "Explain with AI" tap. No
new provider file exists for this reason.

**Reuses `GatedValue<T>`'s established four-state vocabulary.**
`GatedValueForAiExplanation`'s `Limited`/`Unavailable`/
`PermissionRequired` states render through the same
`metricStateLimitedTitle`/`metricStateUnavailableTitle`/
`metricStatePermissionRequiredTitle` strings and icon choices
`database_screen.dart`'s own `_QueryStatsSection` (U34) already
established — not reinvented (each screen keeps its own private
`_GatedMessage` widget; Dart privacy is per-file, so this isn't
literally shared code, but the vocabulary is identical, matching every
prior gated-section's own precedent in this project).

**Risk level shown as its raw wire value**, matching
`policy_inbox_screen.dart`'s own precedent of rendering
`riskClassification` verbatim with no l10n translation switch — avoids
four new risk-label l10n keys entirely.

**Deliberately not rendered**: the raw `evidence_ref`/`candidate_ref`
citation indices — real, internal-to-validation plumbing (SRS FR-AI-005)
with no meaningful surface to a human reader without a "click to jump
to evidence #N" feature this unit doesn't build. Named as real,
separate future work if ever wanted, not silently dropped without
note.

## A real generator bug, found and fixed: `AiExplanation` is the first standalone-emitted type with its own nested `$ref`s

Running `dart run tool/generate_models.dart` for the first time against
U44's new schemas threw `Bad state: conflicting definitions for
AiExplanation`. Every prior wire type in this project is either a flat
struct (no `$ref` fields) or, when composed, is never *also* emitted
standalone at the schema root — `AiExplanation` is the first type that
is both (references `MetricClaim`/`Observation`/`Recommendation`/
`RiskLevel`, and has its own `ai_explanation.schema.json`). Reading the
generator's own dedup check (`_registerNamed`) explained why: schemars
hoists every `$ref`'d sub-type into a `definitions` map at the
*document root* — so the standalone file's own `AiExplanation` node
carries a `definitions` key (its four dependencies), while the
structurally-identical copy nested inside `gated_value_ai_explanation.
schema.json`'s own `definitions.AiExplanation` never does (those same
four dependencies are hoisted to *that* file's root instead). The
generator's `_withoutDescription` helper already stripped `title`/
`description`/`$schema` before comparing two candidate definitions for
equality, but never `definitions` — a real, previously-unexercised gap
in that comparison, not a difference in the actual type. Fixed by also
stripping `definitions` before the equality check (`console/tool/
generate_models.dart`); each sub-type is independently registered and
verified via the same function's own recursion regardless, so this
loses no real verification.

## A real test gotcha, found and fixed: `FakeTransport`'s prefix matching on overlapping paths

`FakeTransport.getRaw` matches a request path against queued prefixes
via `path.startsWith(queuedPrefix)`, in queue-insertion order — a
deliberate design (real path params like `/policy/:id/grant` need
prefix matching, not exact matching, confirmed by reading the file's
own doc comment). `/api/v1/analysis/host` is a real string-prefix of
`/api/v1/analysis/host/explain` — the first time two endpoints in this
test suite have had that literal relationship. Queuing the shorter
path first (the natural order, matching how the screen fetches it)
meant a request to `/explain` matched the *shorter* queue first,
found it already drained by the initial host-verdict fetch, and
returned a genuine (but wrong) "no response scripted" failure — which
is exactly why the very first test run degraded silently to the
`error` stage instead of `loaded`, discovered by tracing real
`initState`/`build`/`_explain` prints, not assumed from reading the
widget code alone. Fixed at the call site (queue the more specific
`/explain` path before its own prefix), not in `FakeTransport` itself
— changing the matcher's semantics would risk breaking every other
test's legitimate use of prefix matching for path params.

## Verification (real, not simulated)

- `console/test/host_analysis_screen_test.dart` (3 new tests): tapping
  "Explain with AI" fires the real request and renders the real
  summary/observations/recommendations/risk/confidence verbatim; a
  real `Unavailable` gated response shows the real reason through the
  shared vocabulary; a real transport-level error shows the real
  failure message, and a working retry after re-queuing a success
  response recovers into the real explanation.
- Both pre-existing tests in the same file pass unmodified — the new
  section never fires its request until its own button is tapped, so
  neither test's un-queued `/explain` endpoint is ever touched.
- `flutter analyze`/`flutter test` clean (60 console tests total, 3
  new); console codegen-drift clean (including the generator fix
  itself, verified by re-running `dart run tool/generate_models.dart`
  clean after the fix); all 3 `.arb` locales updated together, every
  user-visible *label* routed through l10n (the AI's own prose is
  rendered verbatim, the same precedent already used for `query.text`/
  `blockedQuery` elsewhere — it's data, not a UI label).

## Consequences

- `core::ai`'s first console surface exists: an operator can request a
  real, citation-checked AI explanation of the current host verdict
  on demand.
- `console/tool/generate_models.dart`'s dedup check is now correct for
  any future wire type with both a standalone schema *and* its own
  `$ref`'d sub-types — a real, general fix, not specific to
  `AiExplanation`.
- The DB-side explanation (`build_db_bundle`) has no service endpoint
  yet either — real, separate future work, matching the DBMS vein's
  own incremental pairing discipline.
