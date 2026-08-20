//! The policy engine (unit U7, SRS §17-19 FR-POL-001..006, TRS §24-25).
//!
//! **Hard boundary (TRS §24)**: `ActionRequest -> Validated<ActionRequest>
//! -> PolicyApproved` is a typestate chain with private inner fields —
//! [`typestate::Validated`] and [`typestate::PolicyApproved`] are
//! constructible only from inside this module (`pub(in crate::policy)`
//! constructors). A policy bypass — constructing a `PolicyApproved` without
//! actually calling [`validate`]/[`approval::authorize`] — is a compile
//! error, not a review question.
//!
//! **Hard boundary (SRS FR-POL-006)**: the protected-resource registry
//! ([`resource::ProtectedResourceRegistry`]) is consulted by
//! `core::actions`'s executor pipeline as its own dedicated stage — never
//! ad hoc at a call site.
//!
//! **Hard boundary**: nothing here depends on `core::ai` or a future
//! correlation module — every decision in this module is 100%
//! deterministic, the same "no AI in the decision path" precedent
//! `core::analysis`/`core::ai` already established.
//!
//! **Scope note**: this is the policy/approval framework only — zero real
//! (shipped) `ActionKind` implementations live in `core::actions`; those
//! are unit U8's job, gated through what this unit builds (see the U7
//! plan's SCOPE note, same precedent U4-U6 set for their own layers).

pub mod approval;
pub mod error;
pub mod evaluate;
pub mod hash;
pub mod registry;
pub mod request;
pub mod resource;
pub mod risk;
pub mod status;
pub mod target;
pub mod typestate;
pub mod validate;

pub use error::PolicyError;
pub use evaluate::{evaluate, PolicyOutcome, DEFAULT_APPROVAL_TTL_SECONDS};
pub use registry::{ActionTypeEntry, ActionTypeRegistry};
pub use request::ActionRequest;
pub use resource::{ProtectedResourceRegistry, ResourceDescriptor, ResourceKind};
pub use risk::{decide, Environment, PolicyDecision, RiskLevel};
pub use status::ActionStatus;
pub use target::TargetIdentity;
pub use typestate::{PolicyApproved, Validated};
pub use validate::validate;
