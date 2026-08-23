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
//! **FR-CORR-003 note (updated, unit U70 — SRS FR-CORR-003)**: both
//! named consumers are now real. Unit U20 promoted a wire mirror of
//! this module's types into `contracts::correlation`
//! (`CorrelationResult`, `Hypothesis`, `RuledOut`, `RootCause`), and
//! `service::api::v1::analysis`'s `/api/v1/analysis/correlation`
//! endpoint converts this module's own `CorrelationResult` into that
//! wire type before returning it — the console's generated Dart model
//! and l10n strings render exactly that one representation. Unit U70
//! closed the remaining gap: `ai::bundle::build_correlation_bundle`
//! (`build_host_bundle`/`build_db_bundle`'s own sibling) turns this same
//! `CorrelationResult` into the AI provider's evidence bundle, and
//! `service::api::v1::analysis::explain_correlation` wires it to a real
//! `/api/v1/analysis/correlation/explain` endpoint — see docs/adr/0072
//! (named the gap) and docs/adr/0076 (closed it).
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
