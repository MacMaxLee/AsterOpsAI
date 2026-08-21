//! Wire-level contract types shared by every AsterOpsAI component.
//!
//! `contracts` is the single source of truth for anything that crosses the
//! API boundary: it has no workspace-internal dependencies, and every type
//! here derives `schemars::JsonSchema` so `/schemas/*.schema.json` (emitted by
//! the `emit-schemas` binary) stays byte-identical to what the server and
//! console actually exchange. See docs/adr/0002-contract-first-schema-codegen.md.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod analysis;
pub mod capability;
pub mod correlation;
pub mod dbms;
pub mod envelope;
pub mod error;
pub mod health;
pub mod policy;
pub mod security;
pub mod telemetry;
pub mod tuning;

pub use analysis::HostVerdict;
pub use capability::{default_capability_registry, Capability, CapabilityFamily, GatedValue};
pub use correlation::CorrelationResult;
pub use dbms::{LockEdge, QueryStat, SessionInfo, SessionState};
pub use envelope::Envelope;
pub use error::ApiError;
pub use health::{HealthResponse, SelfMetricValue};
pub use policy::{ActionProposalOutcome, PendingActionSummary, ResumableActionSummary};
pub use security::SecurityIncidentSummary;
pub use tuning::{TuningCandidateOutcome, TuningPlanOutcome, TuningPlanSummary};

/// The API's own version, distinct from the crate/package version. Bumped
/// only on a breaking wire-format change.
pub const API_VERSION: &str = "v1";
