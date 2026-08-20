//! The tuning engine (unit U10, SRS §22 FR-TUNE-001..003, TRS §36).
//!
//! **Hard boundary (FR-TUNE-002)**: at most one tuning plan may be in
//! flight per target at a time. Enforced by a real, durable SQLite partial
//! `UNIQUE` index (migrations/V11,
//! `idx_tuning_plans_one_in_flight_per_target`), not an in-memory lock —
//! `repository::tuning::insert_tuning_plan` maps a violation to
//! [`error::TuningError::PlanAlreadyInFlight`]. A concurrent attempt is
//! rejected outright, never queued.
//!
//! **Hard boundary (FR-TUNE-003)**: `AutomationMode::AutoLowRisk` only
//! ever auto-executes a candidate that is `AutoAllowed` by
//! `policy::evaluate` *and* whose action type is `reversible`
//! (`policy::ActionTypeEntry.reversible`) *and* has a locally recorded
//! improved-outcome history (`history::has_improved_history`). All three,
//! every time — see `plan::process_candidate`.
//!
//! **Scope note**: no `service`/console wiring happens in this unit —
//! `AutomationMode::AskBeforeChanges`'s actual human-facing "ask" is a
//! future unit's job; here it only means "propose for real, never
//! auto-authorize" (see `mode::AutomationMode`'s own doc comment).

pub mod candidates;
pub mod error;
pub mod history;
pub mod mode;
pub mod plan;
pub mod profile;

pub use error::TuningError;
pub use mode::AutomationMode;
pub use plan::{start_plan, CandidateOutcome, TuningOutcome, TuningPipeline, TuningPlanStatus};
pub use profile::{desired_state_for, full_cpu_set, DesiredState, TuningProfile};
