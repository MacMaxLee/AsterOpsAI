//! Wire types for the policy approval inbox (unit U13). The first
//! `contracts` type this project's policy engine (U7) has ever had — see
//! docs/adr/0018 for why it stayed `core`-internal until now.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a pending-approval inbox view needs, no more: not the full
/// `actions` row (parameters, evidence, hashes — internal lifecycle
/// detail a listing view has no use for), just enough to let a human
/// decide.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PendingActionSummary {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub action_type: String,
    pub risk_classification: String,
    pub resource_kind: String,
    pub resource_name: String,
    pub requested_by: String,
    pub approval_expires_at: Option<DateTime<Utc>>,
}
