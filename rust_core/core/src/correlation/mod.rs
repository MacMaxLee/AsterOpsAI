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
//! **FR-CORR-003 note**: `CorrelationResult` stays `core`-internal, not
//! a `contracts` wire type, even though FR-CORR-003 describes it as
//! using "the same contract types the console and the AI explanation
//! layer consume" — `HostVerdict`/`DbHealthVerdict` themselves (this
//! unit's own inputs) aren't `contracts` types yet either. Promoting
//! `CorrelationResult` alone would be an inconsistent, orphaned partial
//! schema addition; a future service/console-wiring unit should promote
//! all three together (see docs/adr/0017).
//!
//! **Scope note**: no `service`/console wiring happens in this unit,
//! same incremental-layering precedent every unit since U4 has kept.

pub mod cause;
pub mod confidence;
pub mod correlate;
pub mod hypothesis;

pub use cause::RootCause;
pub use correlate::correlate;
pub use hypothesis::{CorrelationResult, Hypothesis, RuledOut};
