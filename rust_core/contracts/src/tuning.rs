//! Wire types for the tuning plan history view (unit U14). Unlike
//! `contracts::policy::PendingActionSummary`, `target_identity_json` and
//! `candidates_json` are passed through raw rather than parsed into
//! structured fields — `TargetIdentity` has no shared "name" field the
//! way `ResourceDescriptor` does (`Process`/`DbSession` are genuinely
//! different shapes), and `core::tuning::CandidateOutcome` isn't a
//! `contracts` type. See docs/adr/0019.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TuningPlanSummary {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    /// Raw `TargetIdentity` JSON — see this module's own doc comment.
    pub target_identity_json: String,
    pub profile: String,
    pub mode: String,
    pub status: String,
    pub completed_at: Option<DateTime<Utc>>,
    /// Raw JSON array of each candidate action's final outcome — see
    /// this module's own doc comment.
    pub candidates_json: String,
}
