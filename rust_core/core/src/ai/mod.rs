//! The optional AI explanation layer (unit U6, SRS §16 FR-AI-001..005,
//! TRS §22-23).
//!
//! **Hard boundary (SRS FR-AI-002)**: the provider receives only an
//! [`EvidenceBundle`] built from `core::analysis`'s deterministic output —
//! never raw credentials, never an action-execution path.
//!
//! **Hard boundary (SRS FR-AI-005 / TRS §23)**: the citation model —
//! every numeric claim resolves into the bundle's evidence list, every
//! entity reference resolves into its candidate list, a response failing
//! either check is discarded whole — is the documented security boundary.
//! Keyword-based prompt-injection filtering is deliberately not
//! implemented (see `validator.rs`).
//!
//! **Hard boundary (SRS FR-AI-001)**: every caller outside this module
//! should use [`try_explain`] rather than [`AiProvider::explain`] directly
//! — it degrades to `None` on any failure (absent, unreachable, slow,
//! garbage output), never propagates or panics.
//!
//! **Scope note**: this is the provider/validator/prompt layer only — not
//! wired into a live poll loop, `service`'s HTTP surface, or the console,
//! and does not persist its output (see the U6 plan's SCOPE note, same
//! precedent U4/U5 set for `core::dbms`/`core::analysis`).

pub mod bundle;
pub mod ollama;
pub mod prompt;
pub mod provider;
pub mod schema;
pub mod validator;

pub use bundle::{build_db_bundle, build_host_bundle, Candidate, EvidenceBundle, EvidenceItem};
pub use ollama::OllamaProvider;
pub use provider::{try_explain, AiError, AiProvider, AiProviderConfig};
pub use schema::{
    AiExplanation, MetricClaim, Observation, RawAiExplanation, Recommendation, RiskLevel,
};
