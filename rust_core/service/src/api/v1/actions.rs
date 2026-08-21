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
use axum::extract::{Extension, State};
use axum::Json;
use chrono::Utc;
use contracts::{ActionProposalOutcome, ApiError};
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
