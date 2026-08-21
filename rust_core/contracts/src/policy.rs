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

/// Unit U26's real, immediate result of proposing an action outside the
/// tuning pipeline (`POST /api/v1/actions/propose`) — mirrors
/// `core::policy::PolicyOutcome`'s three variants (`AutoAllowed`,
/// `PendingApproval`, `Denied`) field-for-field, the same way
/// `TuningPlanOutcome` mirrors `TuningOutcome`. `status` is one of
/// `"AUTO_ALLOWED"`, `"PENDING_APPROVAL"`, `"DENIED"`;
/// `approval_expires_at` is set only for `PENDING_APPROVAL`, `reason`
/// only for `DENIED`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionProposalOutcome {
    pub row_id: i64,
    pub status: String,
    pub approval_expires_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

/// Unit U29: what `GET /api/v1/actions/resumable` returns — the one
/// `EXECUTED`, not-yet-rolled-back action against a given target, if
/// any. A list of 0 or 1 items, not `Option<T>`: the envelope treats
/// "success with null data" as malformed, so "nothing to resume" is
/// expressed as an empty list, the same convention
/// `PendingActionSummary`'s own listing already uses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResumableActionSummary {
    pub row_id: i64,
    pub action_type: String,
    pub executed_at: DateTime<Utc>,
}
