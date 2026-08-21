# 0049 — Service: AI Explanation Wiring (Host Analysis)

## Status

Accepted (unit U44).

## Context

`core::ai` (SRS FR-AI-001..005, TRS §22-23) — provider trait, Ollama
HTTP client, prompt construction, closed-schema validation with
citation checking — has been fully built since unit U6, but had zero
references anywhere in `service/src`, confirmed by grep during a
background gap audit of every ADR's own named future-work items. Its
own module doc comment already said as much: *"not wired into a live
poll loop, `service`'s HTTP surface, or the console... same precedent
U4/U5 set for `core::dbms`/`core::analysis`"* — the same
"fully-built-but-unwired" shape the DBMS vein (ADR 0036 onward)
started from. This unit begins the analogous AI-wiring vein with its
own smallest first slice: host-analysis explanation only, the same
incremental discipline U31 used to start DBMS with sessions+locks
rather than all 12 methods at once.

## Decision

**`service/src/ai_config.rs`** (NEW), mirroring `dbms_config.rs`'s own
plain-env-var, all-or-nothing precedent: `resolve_ai_config()` reads
`$ASTEROPS_AI_MODEL` (required — its presence is what "AI is
configured" means, exactly like `dbms_config`'s host+password gate;
guessing a default model name would just fail confusingly at the
provider), `$ASTEROPS_AI_BASE_URL`/`$ASTEROPS_AI_TIMEOUT_SECS`
(optional, falling back to `AiProviderConfig::new`'s own defaults).

**`AppState` gains `ai_provider: Option<Arc<dyn AiProvider>>`**, the
same shape as `dbms_adapter`. `main.rs` constructs it as
`resolve_ai_config().map(|cfg| Arc::new(OllamaProvider::new(cfg)))`.
This is `AppState::new`'s 9th positional argument, so every existing
`AppState::new(...)` call site (10 test files + `main.rs`) needed a
new trailing argument — the same mechanical cost every prior arity
change in this project (`dbms_adapter`, `policy_environment`) already
paid.

**New `contracts::ai` module**: `MetricClaim`, `Observation`,
`Recommendation`, `RiskLevel`, `AiExplanation` — deliberately parallel
to `core::ai::schema`'s types, not re-exported, matching every other
`contracts` module's convention.

**`GET /api/v1/analysis/host/explain`** reuses `compute_host_verdict`
(already shared by `host`/`correlation` since U19/U20) unchanged, and
`core::ai::build_host_bundle` for the evidence bundle. Three real,
distinguishable outcomes, mirroring `query_stats`' own precedent (ADR
0038) exactly:
- No provider configured → real `503 Unavailable`, the same category
  `dbms_adapter`'s own absence already uses.
- A configured provider whose `try_explain` call degrades to `None`
  (absent, unreachable, timeout, garbage output, an unresolved
  citation — `try_explain`'s own contract at the `core` level
  deliberately discards which one, SRS FR-AI-001) → real `200 OK` with
  `GatedValue::Unavailable { reason: "AI explanation unavailable" }` —
  a generic reason, because inventing a more specific one here would
  be dishonest: `core::ai` itself never surfaces the specific cause
  past its own boundary.
- A real explanation → real `200 OK` with `GatedValue::Supported {
  value: <wire AiExplanation> }`.

## Verification technique: a real local TCP double, not a mocked provider

`core/tests/ai_ollama_provider_test.rs`'s own doc comment already
established the real, no-genuine-Ollama-needed technique this project
uses: a real local `TcpListener` bound to an ephemeral port, standing
in for a real Ollama daemon — real sockets, real bytes, never a mocked
`AiProvider` trait implementation. `service/tests/ai_endpoints.rs`
reuses that exact technique rather than mocking `AiProvider` at the
service layer — the small helper functions (`one_shot_server`,
`closed_port`, `http_ok_json_body`, `ollama_envelope`) are
reimplemented directly in the new test file rather than imported,
since `core/tests/ai/common/mod.rs` is test-only code in a different
package and can't be shared across the boundary (the same constraint
ADR 0025 already named for `support` modules).

## Verification (real, not simulated)

- `service/tests/ai_endpoints.rs` (3 new tests):
  `explain_host_is_unavailable_without_an_ai_provider` (503, no
  provider configured); `explain_host_reports_a_real_unavailable_
  state_as_200_ok_when_the_provider_is_unreachable` (a real closed
  port — genuine connection-refused, not a guessed timeout — 200 +
  `GatedValue::Unavailable`); `explain_host_returns_a_real_supported_
  explanation_from_a_real_provider_round_trip` (a real one-shot TCP
  server returning a real, well-formed Ollama-shaped response — 200 +
  `GatedValue::Supported`, asserting the real summary/risk/confidence
  echo back verbatim). The well-formed response deliberately carries
  zero metric/candidate citations, so `core::ai::validator::validate`
  passes regardless of the real bundle's own live evidence/candidate
  contents — a real, not fabricated, way to keep the test
  environment-independent.
- `service/src/ai_config.rs` (5 unit tests, mirroring `dbms_config.rs`'s
  own test style): no-model/empty-model → unconfigured; a model alone
  fills conventional defaults; explicit fields override; an unparsable
  timeout falls back rather than erroring.
- Full workspace `cargo build/test/clippy -D warnings/fmt --check`
  clean (all pre-existing tests across every one of the 10 updated
  `AppState::new` call sites still pass unchanged); both grep gates
  clean; schema-drift clean (5 new types + `GatedValue<AiExplanation>`
  + envelope committed); `cargo deny check`/`cargo audit` clean; no
  leaked processes (including the new test doubles' `TcpListener`s).

## Consequences

- `core::ai` has its first real HTTP surface — a whole feature area
  that was fully built and completely dormant now has one live,
  end-to-end path: telemetry → `classify_host` → `EvidenceBundle` →
  Ollama → validated `AiExplanation` → wire response.
- `build_db_bundle` (the DB-side explanation) is real, separate future
  work — same incremental pairing discipline the DBMS vein itself
  used; host-analysis alone is the complete, smallest first slice, not
  a half-finished pair.
- No console UI for this endpoint exists yet — real, separate future
  work, matching the established service-then-console pattern.
- `AppState`'s constructor now takes 9 positional arguments. A future
  unit adding a 10th subsystem should expect the same mechanical
  cost across every existing call site — a real, known, accepted
  tradeoff of this project's plain-positional-constructor style, not
  reconsidered here.
