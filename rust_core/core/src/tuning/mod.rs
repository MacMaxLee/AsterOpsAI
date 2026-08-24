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
//! `AutomationMode::AskBeforeChanges`'s own human-facing "ask" was once
//! described here as "a future unit's job" — that unit already
//! happened: `process_candidate` routes an `AskBeforeChanges` candidate
//! through the same `policy::evaluate` every other action type uses, so
//! it lands as a real `PENDING_APPROVAL` row in the shared policy inbox
//! (`GET /api/v1/policy/pending`, `console/lib/screens/policy_inbox_
//! screen.dart`) exactly like any other proposed action — proven by
//! `core/tests/tuning_plan_recommend_and_ask_test.rs`'s own
//! `ask_before_changes_proposes_for_real_but_never_authorizes`. No
//! tuning-specific console affordance was ever needed, since the inbox
//! is already generic over which subsystem proposed the action.

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
