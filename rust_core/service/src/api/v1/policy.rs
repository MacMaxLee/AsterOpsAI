//! Unit U13's policy approval inbox: list pending actions, grant one,
//! reject one. Unit U22 made `grant` real: it now calls `authorize()` and
//! `actions::execute()` for real (both already fully built and tested at
//! the `core` level, unit U7/U8) — approving something here genuinely
//! applies it, not just marks it approved. See docs/adr/0027.

use ai_ops_core::actions::{self, ActionError, TargetVerifier};
use ai_ops_core::policy::{approval, PolicyError, ResourceDescriptor, TargetIdentity};
use ai_ops_core::repository::{self, get_action, PolicyActionRow};
use axum::extract::{Extension, Path, State};
use axum::Json;
use chrono::Utc;
use contracts::{ApiError, PendingActionSummary};
use serde::Deserialize;
use serde_json::Value;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

/// The one real `TargetVerifier` (unit U8's `core::actions::host`,
/// Linux-only — `core::actions::host` has no Windows/macOS
/// implementation yet). On other platforms `AppState.action_registry` is
/// always empty (`main.rs`'s own Linux gate), so `authorize()` always
/// fails with a real `UnknownActionType` before a verifier would ever be
/// consulted — this stub exists purely so the handler compiles
/// cross-platform (CLAUDE.md's CI matrix), never to be genuinely invoked.
#[cfg(not(target_os = "linux"))]
struct UnsupportedTargetVerifier;

#[cfg(not(target_os = "linux"))]
impl TargetVerifier for UnsupportedTargetVerifier {
    fn verify(&self, _expected: &TargetIdentity) -> Result<(), ActionError> {
        Err(ActionError::CapabilityUnavailable(
            "host actions are not implemented on this platform yet".to_string(),
        ))
    }
}

#[cfg(target_os = "linux")]
fn target_verifier() -> impl TargetVerifier {
    ai_ops_core::actions::host::ProcessTargetVerifier
}

#[cfg(not(target_os = "linux"))]
fn target_verifier() -> impl TargetVerifier {
    UnsupportedTargetVerifier
}

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
        // The action type itself has no registered entry at all (unit
        // U22) — a real, server-wide capability gap (e.g. this platform
        // has no `core::actions::host` implementation yet), distinct
        // from `ActionError::CapabilityUnavailable` below, which fires
        // *after* a real entry was found, for a request-specific reason
        // (its target vanished, etc.) — that one is a 400, this is a 501.
        PolicyError::UnknownActionType(action_type) => {
            ApiError::Unsupported(format!("no registered action type: {action_type}"))
        }
        other => {
            tracing::error!(error = %other, "unexpected policy error");
            ApiError::Internal
        }
    }
}

/// Unlike `policy_error_to_api`, every branch here reflects a real,
/// already-applied-or-attempted execution outcome (unit U22) — the row
/// has moved past `APPROVED` by the time any of these can happen, so
/// these are reported honestly as failures rather than folded into a
/// generic success (this project's consistent no-fabricated-success
/// discipline).
fn action_error_to_api(err: ActionError) -> ApiError {
    match err {
        ActionError::Policy(policy_err) => policy_error_to_api(policy_err),
        // A real discovery building this unit: `check_capability()`
        // implementations (e.g. `SetProcessCpuAffinityAction`'s own)
        // directly probe the live target as part of the check itself —
        // so this fires for a request-specific reason (the target
        // vanished between grant and execute), not "this server can't do
        // this at all." `core::actions::ActionError` doesn't distinguish
        // the two cases in its own type, but by the time `execute()` is
        // running, the action type *was* found in the registry, so a 400
        // (this specific request can't be fulfilled) is the honest
        // mapping here, not a 501 (the server doesn't implement this).
        ActionError::CapabilityUnavailable(reason) => ApiError::BadRequest(reason),
        other => {
            tracing::warn!(error = %other, "action execution failed");
            ApiError::BadRequest(other.to_string())
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
