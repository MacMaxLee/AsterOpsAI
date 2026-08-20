# 0011 — AI Provider: HTTP Client Choice, Closed-Schema Validator, and Testing Without a Live Ollama

## Status

Accepted (unit U6).

## Context

U6 implements SRS §16 (FR-AI-001..005) and TRS §22-23: an optional
`AiProvider` (Ollama by default) that turns U5's deterministic evidence
into a plain-language explanation, under a hard, mechanically-enforced
citation boundary. TRS §23 explicitly names this unit as where that
boundary's design gets written up ("the closed output schema plus
candidate-id indirection is the documented security boundary — see the
corresponding ADR from unit U6"). Several concrete decisions came up that
neither SRS/TRS pins nor any earlier unit resolved, each settled by reading
the actual code/environment rather than guessed.

## Decision

**HTTP client: `hyper`'s client feature directly, not `reqwest`, and no TLS
backend at all.** Confirmed before writing any code: no HTTP client crate
exists anywhere in the workspace; `service` already depends on `hyper`
(server-side, ADR 0001's UDS transport) and `hyper-util`, so extending
those same crates with client-side features (`hyper` `client`+`http1`,
`hyper-util` `tokio` for the `TokioIo` adapter, `http-body-util` for
request/response bodies, `http` for `Request` construction, `bytes`) reuses
an already-vetted dependency instead of pulling in `reqwest`'s much larger
tree for one endpoint. Ollama's own default and every documented deployment
is plain `http://` (typically loopback or LAN) — adding a TLS backend now
would repeat U2/U4's `cmake`/cross-compile friction (`aws_lc_rs`, avoided
there via `ring`) for a capability nothing in SRS/TRS asks for.
`OllamaProvider::explain` opens a fresh `TcpStream` + `hyper::client::conn::
http1::handshake` per call — no connection pool — since these calls are
infrequent and a one-shot connection keeps the client trivial (no
keep-alive bookkeeping, no idle-connection eviction). A future remote/HTTPS
Ollama endpoint is a real, flagged gap for a later unit — the same
scope-down pattern ADR 0009 used for `TlsMode::VerifyCa`/`VerifyFull`.

**No fake `AiProvider` trait implementation — every test runs the real
`OllamaProvider` against a real local `TcpListener`.** Checked first:
`PlatformAdapter`, `DbmsAdapter`, `CredentialStore` each have exactly one
real implementation in this codebase; test coverage elsewhere always comes
from standing up a real dependency (U4's extracted-PostgreSQL binaries),
never a hand-written fake trait impl. Ollama itself isn't installed or
reachable in this sandbox (`which ollama` finds nothing, a direct
`/dev/tcp/127.0.0.1/11434` probe gets connection-refused, no `~/.ollama`) —
outbound internet works, so a real install is possible later, but not
available now. `core/tests/ai/common/mod.rs`'s `one_shot_server`/
`slow_one_shot_server`/`closed_port` helpers are real `tokio::net::
TcpListener`s this test suite controls, writing real HTTP/1.1 response
bytes — the *real* `OllamaProvider` HTTP client is what's under test in
every case (connection-refused, timeout, non-success status, malformed
JSON, schema-violating JSON, well-formed happy path), not a mocked trait.
This is a residual gap, not a substitute: it proves the client's real wire
behavior against Ollama's documented shape, but never proves a genuine
Ollama daemon actually returns that shape. Closes once someone installs a
real Ollama and points `AiProviderConfig::base_url` at it — worth a
`scripts/setup-test-ollama.sh` analog to U4's PostgreSQL script if/when
that becomes a priority, not built this unit.

**Ollama's real, documented `/api/generate` wire shape is assumed: a
two-level JSON parse.** The outer envelope is `{"response": "<string>",
"done": bool, ...}` (other fields ignored — plain `serde::Deserialize`,
matching FR-AI-003's "discard unknown fields" exactly); `response` is
itself a JSON-encoded *string* (the model's completion under `format:
"json"` mode), parsed a second time into `RawAiExplanation`. Getting this
two-level shape wrong would show up immediately as an `InvalidJson` error
against a real Ollama — flagged as the one assumption in this design that
real-daemon testing (once available) should confirm first.

**Numerics are confined to a dedicated `MetricClaim{value, evidence_ref}`
field, never embedded in free `text` prose.** FR-AI-005 requires the
citation check to be "testable by an automated validator, not by prompt
wording alone" — a validator can only mechanically check a structured
field, not extract numbers out of prose (that would mean regex/NLP
guessing, itself untestable in the way the requirement demands). This
shapes the whole closed schema: `Observation`/`Recommendation` both carry a
`metrics: Vec<MetricClaim>` alongside their prose `text`; only
`Recommendation` carries an optional `candidate_ref` (FR-AI-004's "any
action-shaped content... references a candidate_id" — recommendations are
the action-shaped content, observations are not). `validator::validate`
walks every `MetricClaim`'s `evidence_ref` against `EvidenceBundle.evidence`
by `id` (not raw index, so bundle ordering isn't load-bearing) and every
`Some(candidate_ref)` against `EvidenceBundle.candidates` the same way;
`risk` is checked against a fixed four-value allow-list and `confidence`
against `0.0..=1.0`. **Any single failure discards the whole response** —
proven directly by `ai_validator_test.rs`'s
`one_bad_citation_among_many_good_ones_rejects_whole_response`, which buries
one bad `evidence_ref` among several otherwise-valid claims and asserts the
entire `validate()` call still errors.

**`try_explain` is the one entry point every future caller should use**
(SRS FR-AI-001) — it wraps `AiProvider::explain`, logs a warning, and
returns `None` on any error (connect failure, timeout, non-success status,
malformed JSON at either parse level, a citation that doesn't resolve),
mirroring `repository::try_persist_telemetry_snapshot`'s established
"never propagate, log and degrade" shape. `ai_degradation_test.rs` proves
this holds for an absent provider, a slow one (confirms `try_explain`
returns well within the configured timeout, not just eventually), and
garbage HTTP/JSON output — no panic, no unbounded block, in every case.

**No persistence, no correlation-engine input, no service/console wiring
this unit** — matching U4/U5's own SCOPE precedent. `EvidenceBundle` is
built directly from a single `analysis::HostVerdict`/`DbHealthVerdict` via
`build_host_bundle`/`build_db_bundle`; cross-layer correlation (SRS §31/
TRS §20) is a distinct, still-unbuilt unit that would feed richer
hypotheses into a bundle like this one, not something this unit needed to
wait for or build itself.

## Consequences

- The two-level Ollama envelope assumption and the exact `/api/generate`
  request shape (`model`/`system`/`prompt`/`stream`/`format` fields) are
  the one part of this design not yet verified against a genuinely running
  Ollama daemon — real end-to-end verification is a natural follow-up once
  Ollama is installed, not a blocker for this unit's own DoD.
- `AiProviderConfig.model` has no hardcoded default — every caller must
  say which model it expects pulled locally; a caller that doesn't will get
  a clear provider-side HTTP error (surfaced as `AiError::HttpStatus` or
  `AiError::InvalidJson` depending on how Ollama reports a missing model),
  not a silent wrong-model call.
- `RiskLevel` is advisory-only and entirely separate from the policy
  engine's own authoritative risk classification (SRS §17, a later unit) —
  nothing in `core::ai` gates or influences an action decision; the AI
  layer has no execution path at all (FR-AI-002).
- A remote/HTTPS Ollama endpoint remains unsupported until a later unit
  adds a TLS backend and a certificate-verification story, the same real,
  flagged gap pattern as `TlsMode::VerifyCa`/`VerifyFull` in ADR 0009.
