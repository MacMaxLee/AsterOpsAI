//! Unit U26's `POST /actions/propose`: a second, generic caller of
//! `core::policy::{validate, evaluate}` — plain, non-tuning-specific
//! functions `core::tuning::plan.rs` itself is just one caller of.
//! Lets any registered action type (today: `security.suspend_process`,
//! unit U11/TRS §37, previously registered nowhere) be proposed against
//! a real process target directly, without going through the tuning
//! profile/candidate-diffing machinery `/tuning/start` is built around.
//! See docs/adr/0031.

use ai_ops_core::policy::{
    evaluate, validate, ActionRequest, PolicyOutcome, ResourceDescriptor, ResourceKind,
    TargetIdentity,
};
use ai_ops_core::repository;
use axum::extract::{Extension, Query, State};
use axum::Json;
use chrono::Utc;
use contracts::{ActionProposalOutcome, ApiError, ResumableActionSummary};
use serde::Deserialize;
use serde_json::Value;

use crate::actions::policy_error_to_api;
use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

/// Process-only, matching `StartPlanRequest`'s own precedent (unit U23):
/// no `TargetIdentity::Device`/`DbSession` variant is offered — not
/// silently unsupported, just not exposed by this wire contract yet.
/// `parameters` defaults to an empty object when omitted (most action
/// types, e.g. `security.suspend_process`, take none at all).
#[derive(Debug, Deserialize)]
pub struct ProposeActionRequest {
    pub action_type: String,
    pub pid: u32,
    pub start_time_ticks: u64,
    pub resource_name: String,
    #[serde(default)]
    pub parameters: Option<Value>,
    pub requested_by: String,
}

pub async fn propose(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Json(body): Json<ProposeActionRequest>,
) -> ApiResponse<ActionProposalOutcome> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "action proposals are not available: repository layer did not start".to_string(),
            )
        })?;

        let request = ActionRequest {
            action_type: body.action_type,
            target: TargetIdentity::Process {
                pid: body.pid,
                start_time_ticks: body.start_time_ticks,
            },
            resource: ResourceDescriptor {
                kind: ResourceKind::Process,
                name: body.resource_name,
            },
            parameters: body.parameters.unwrap_or_else(|| serde_json::json!({})),
            requested_by: body.requested_by,
        };

        let validated = validate(request, &state.action_registry).map_err(policy_error_to_api)?;
        let outcome = evaluate(
            validated,
            state.policy_environment,
            &state.action_registry,
            &repo,
            Utc::now(),
        )
        .await
        .map_err(policy_error_to_api)?;

        Ok(match outcome {
            PolicyOutcome::AutoAllowed { row_id } => ActionProposalOutcome {
                row_id,
                status: "AUTO_ALLOWED".to_string(),
                approval_expires_at: None,
                reason: None,
            },
            PolicyOutcome::PendingApproval { row_id, expires_at } => ActionProposalOutcome {
                row_id,
                status: "PENDING_APPROVAL".to_string(),
                approval_expires_at: Some(expires_at),
                reason: None,
            },
            PolicyOutcome::Denied { row_id, reason } => ActionProposalOutcome {
                row_id,
                status: "DENIED".to_string(),
                approval_expires_at: None,
                reason: Some(reason),
            },
        })
    }
    .await;

    ApiResponse::new(request_id, result)
}

#[derive(Debug, Deserialize)]
pub struct ResumableActionsQuery {
    pub pid: u32,
    pub start_time_ticks: u64,
}

/// Unit U29: the discovery step a real "Resume" affordance needs —
/// `POST /policy/{id}/rollback` (unit U28) requires a row id, and
/// nothing else finds one for a given process (the Policy inbox only
/// ever lists `PENDING_APPROVAL` rows; a granted suspend moves to
/// `EXECUTED` and disappears from it). A list of 0 or 1 items, not
/// `Option<T>` — see `contracts::ResumableActionSummary`'s own doc
/// comment for why. See docs/adr/0034.
pub async fn resumable(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Query(query): Query<ResumableActionsQuery>,
) -> ApiResponse<Vec<ResumableActionSummary>> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "action proposals are not available: repository layer did not start".to_string(),
            )
        })?;

        let target = TargetIdentity::Process {
            pid: query.pid,
            start_time_ticks: query.start_time_ticks,
        };
        let target_identity_json = serde_json::to_string(&target).map_err(|err| {
            tracing::error!(error = %err, "failed to serialize target identity");
            ApiError::Internal
        })?;

        let row = repository::find_resumable_action(
            &repo,
            &target_identity_json,
            query.start_time_ticks as i64,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "find_resumable_action failed");
            ApiError::Internal
        })?;

        Ok(row
            .into_iter()
            .map(|row| ResumableActionSummary {
                row_id: row.id,
                action_type: row.action_type,
                executed_at: row.executed_at.unwrap_or(row.created_at),
            })
            .collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}
