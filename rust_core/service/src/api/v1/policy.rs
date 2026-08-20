//! Unit U13's policy approval inbox: list pending actions, grant one,
//! reject one. Never calls `authorize()`/`execute()` — this surface only
//! lets a human see and decide; actually running an approved action is
//! real, separate work for a future unit (see docs/adr/0018).

use ai_ops_core::policy::{approval, PolicyError, ResourceDescriptor};
use ai_ops_core::repository::{self, PolicyActionRow};
use axum::extract::{Extension, Path, State};
use axum::Json;
use chrono::Utc;
use contracts::{ApiError, PendingActionSummary};
use serde::Deserialize;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

fn policy_error_to_api(err: PolicyError) -> ApiError {
    match err {
        PolicyError::NotFound(_) => ApiError::NotFound,
        PolicyError::WrongStatus {
            id,
            expected,
            actual,
        } => ApiError::BadRequest(format!(
            "action {id} is not in the expected status: expected {expected}, actual {actual}"
        )),
        PolicyError::Expired(id) => {
            ApiError::BadRequest(format!("action {id}'s approval has expired"))
        }
        other => {
            tracing::error!(error = %other, "unexpected policy error");
            ApiError::Internal
        }
    }
}

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
            .map_err(policy_error_to_api)
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
