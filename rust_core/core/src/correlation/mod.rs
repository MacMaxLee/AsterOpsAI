//! Correlation Engine (unit U12, SRS §31 FR-CORR-001..003, TRS §20).
//!
//! **Hard boundary (FR-CORR-001)**: pure, deterministic combination of
//! `analysis::HostVerdict` + `analysis::DbHealthVerdict` — no I/O, no AI
//! dependency of any kind, same boundary `core::analysis`'s own
//! FR-PERF-005 note and `core::security::detector`'s own FR-SEC-001 note
//! already establish.
//!
//! **Scope note**: consumes `core::analysis`'s already-classified output
//! types, never raw telemetry directly — the same boundary
//! `analysis::mod.rs`'s own module doc already forward-referenced this
//! unit with ("Cross-layer correlation... is a distinct, later unit that
//! consumes this module's output types as input").
//!
//! **FR-CORR-003 note (updated, unit U66 — this note was stale)**: unit
//! U20 did promote a wire mirror of this module's types into
//! `contracts::correlation` (`CorrelationResult`, `Hypothesis`,
//! `RuledOut`, `RootCause`), and `service::api::v1::analysis`'s real
//! `/api/v1/analysis/correlation` endpoint converts this module's own
//! `CorrelationResult` into that wire type before returning it — the
//! same one representation the console's generated Dart model and l10n
//! strings render. The console half of FR-CORR-003 is real and wired.
//! What's still genuinely missing: FR-CORR-003 also names "the AI
//! explanation layer" as a consumer, and no `build_correlation_bundle`-
//! style AI-explanation feature for correlation output exists yet
//! (unlike `ai::bundle::build_host_bundle`/`build_db_bundle`) — a real,
//! separate, not-yet-built feature, named here rather than left as a
//! stale claim that nothing was wired (see docs/adr/0072).
//!
//! **Scope note**: no `service`/console wiring happens in this module
//! itself — `core::correlation::CorrelationResult` stays the pure
//! computation type; `service` owns the conversion to the wire type,
//! same incremental-layering precedent every unit since U4 has kept.

pub mod cause;
pub mod confidence;
pub mod correlate;
pub mod hypothesis;

pub use cause::RootCause;
pub use correlate::correlate;
pub use hypothesis::{CorrelationResult, Hypothesis, RuledOut};
