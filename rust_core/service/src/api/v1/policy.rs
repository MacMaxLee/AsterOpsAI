//! Unit U13's policy approval inbox: list pending actions, grant one,
//! reject one. Unit U22 made `grant` real: it now calls `authorize()` and
//! `actions::execute()` for real (both already fully built and tested at
//! the `core` level, unit U7/U8) — approving something here genuinely
//! applies it, not just marks it approved. See docs/adr/0027.

use ai_ops_core::actions;
use ai_ops_core::policy::{approval, ResourceDescriptor, TargetIdentity};
use ai_ops_core::repository::{self, get_action, PolicyActionRow};
use axum::extract::{Extension, Path, State};
use axum::Json;
use chrono::Utc;
use contracts::{ApiError, PendingActionSummary};
use serde::Deserialize;
use serde_json::Value;

use crate::actions::{action_error_to_api, policy_error_to_api, target_verifier};
use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

/// Only what a listing view needs — never the full row (parameters,
/// evidence, hashes are internal lifecycle detail, see
/// `contracts::policy`'s own doc comment).
fn to_summary(row: &PolicyActionRow) -> Result<PendingActionSummary, ApiError> {
    let resource: ResourceDescriptor = serde_json::from_str(&row.resource_descriptor_json)
        .map_err(|err| {
            tracing::error!(error = %err, row_id = row.id, "malformed resource_descriptor_json");
            ApiError::Internal
        })?;
    let resource_kind = serde_json::to_value(resource.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "UNKNOWN".to_string());
    Ok(PendingActionSummary {
        id: row.id,
        created_at: row.created_at,
        action_type: row.action_type.clone(),
        risk_classification: row.risk_classification.clone(),
        resource_kind,
        resource_name: resource.name,
        requested_by: row.requested_by.clone(),
        approval_expires_at: row.approval_expires_at,
    })
}

pub async fn pending(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<PendingActionSummary>> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "policy inbox not available: repository layer did not start".to_string(),
            )
        })?;
        let rows = repository::list_pending_policy_actions(&repo)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "list_pending_policy_actions failed");
                ApiError::Internal
            })?;
        rows.iter().map(to_summary).collect::<Result<Vec<_>, _>>()
    }
    .await;

    ApiResponse::new(request_id, result)
}

#[derive(Debug, Deserialize)]
pub struct GrantRequest {
    pub granted_by: String,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub rejected_by: String,
}

#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub rolled_back_by: String,
}

pub async fn grant(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Path(id): Path<i64>,
    Json(body): Json<GrantRequest>,
) -> ApiResponse<()> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "policy inbox not available: repository layer did not start".to_string(),
            )
        })?;
        approval::grant(&repo, id, body.granted_by, Utc::now())
            .await
            .map_err(policy_error_to_api)?;

        // The row itself already carries exactly what was proposed —
        // `authorize()` re-checks the caller-supplied copy against it
        // (TRS §25's mutation/replay protection), so this round-trips the
        // row's own stored JSON straight back in, not a fresh value.
        let row = get_action(&repo, id)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "get_action failed after grant");
                ApiError::Internal
            })?
            .ok_or(ApiError::NotFound)?;
        let target: TargetIdentity =
            serde_json::from_str(&row.target_identity_json).map_err(|err| {
                tracing::error!(error = %err, row_id = id, "malformed target_identity_json");
                ApiError::Internal
            })?;
        let parameters: Value = serde_json::from_str(&row.parameters_json).map_err(|err| {
            tracing::error!(error = %err, row_id = id, "malformed parameters_json");
            ApiError::Internal
        })?;
        let resource: ResourceDescriptor = serde_json::from_str(&row.resource_descriptor_json)
            .map_err(|err| {
                tracing::error!(error = %err, row_id = id, "malformed resource_descriptor_json");
                ApiError::Internal
            })?;

        let approved = approval::authorize(
            &repo,
            id,
            &target,
            &parameters,
            &resource,
            &state.action_registry,
            &state.action_context,
            Utc::now(),
        )
        .await
        .map_err(policy_error_to_api)?;

        actions::execute(
            approved,
            &target_verifier(),
            &state.protected_resources,
            &repo,
        )
        .await
        .map(|_executed| ())
        .map_err(action_error_to_api)
    }
    .await;

    ApiResponse::new(request_id, result)
}

/// Unit U28: the first way to reach `core::actions::rollback_by_row_id`
/// over HTTP — the only way anything in the running product can undo a
/// real, already-executed action (e.g. resume a process unit U27's
/// suspend proposal really froze). Pure wiring at this layer; the real
/// reconstruction logic lives entirely in `core` (see docs/adr/0033).
pub async fn rollback(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Path(id): Path<i64>,
    Json(body): Json<RollbackRequest>,
) -> ApiResponse<()> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "policy inbox not available: repository layer did not start".to_string(),
            )
        })?;
        actions::rollback_by_row_id(
            id,
            &state.action_registry,
            &state.action_context,
            &target_verifier(),
            body.rolled_back_by,
            &repo,
        )
        .await
        .map(|_rollback_row_id| ())
        .map_err(action_error_to_api)
    }
    .await;

    ApiResponse::new(request_id, result)
}

pub async fn reject(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Path(id): Path<i64>,
    Json(body): Json<RejectRequest>,
) -> ApiResponse<()> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "policy inbox not available: repository layer did not start".to_string(),
            )
        })?;
        approval::reject(&repo, id, body.rejected_by, Utc::now())
            .await
            .map_err(policy_error_to_api)
    }
    .await;

    ApiResponse::new(request_id, result)
}
