//! Stage 3 of TRS §26's pipeline: approval check (TRS §25's approval
//! lifecycle). `grant` simulates an operator's explicit approval
//! (PENDING_APPROVAL -> APPROVED); `authorize` is where single-use,
//! expiry, replay, parameter-mutation, and target-change are all enforced
//! — every one of TRS §25's named rejected states maps to a specific,
//! testable failure path here.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::actions::ActionContext;
use crate::repository::{
    get_action, record_audit_event, transition_action, NewAuditEvent, RepositoryHandle,
    TransitionPatch,
};

use super::error::{map_transition_err, PolicyError};
use super::hash::compute_parameters_hash;
use super::registry::ActionTypeRegistry;
use super::request::ActionRequest;
use super::resource::ResourceDescriptor;
use super::status::ActionStatus;
use super::target::TargetIdentity;
use super::typestate::PolicyApproved;

/// PENDING_APPROVAL -> APPROVED. Rejects (via the CAS's expiry check) an
/// already-expired proposal, and (via the CAS's status check) anything not
/// currently PENDING_APPROVAL — including a row already granted once
/// (replay of `grant` itself).
pub async fn grant(
    handle: &RepositoryHandle,
    row_id: i64,
    granted_by: impl Into<String>,
    now: DateTime<Utc>,
) -> Result<(), PolicyError> {
    let granted_by = granted_by.into();
    transition_action(
        handle,
        row_id,
        ActionStatus::PendingApproval.as_str(),
        ActionStatus::Approved.as_str(),
        now,
        true,
        TransitionPatch {
            approved_by: Some(granted_by.clone()),
            ..Default::default()
        },
    )
    .await
    .map_err(map_transition_err)?;

    // FR-POL-005: an operator's approval decision is itself audited, not
    // just implied by the row's own `approved_by` column — a row that's
    // later denied/expired/replayed still leaves a record that it was
    // granted at all.
    record_audit_event(
        handle,
        NewAuditEvent {
            ts: now,
            event_type: "policy.granted".to_string(),
            actor: granted_by,
            summary: format!("action {row_id} approved"),
            detail_json: serde_json::json!({ "row_id": row_id }).to_string(),
        },
    )
    .await?;
    Ok(())
}

/// Consumes an APPROVED (or AUTO_ALLOWED, which needs no separate `grant`)
/// row, producing the one way to construct a [`PolicyApproved`] — the
/// approval-check pipeline stage. `target`/`parameters`/`resource` are
/// re-supplied by the caller and checked against exactly what was
/// proposed: a mismatch on any of them (TRS §25's "different target" /
/// "mutated parameters" — `resource` gets the identical treatment since
/// it's exactly the value `actions::execute`'s protected-resource stage
/// later trusts; letting it drift from what was actually proposed would
/// make FR-POL-006's protection check checkable against a relabeled
/// resource instead of the real one) is rejected as
/// [`PolicyError::ApprovalMismatch`], never silently allowed through with
/// the new values. The status transition to `EXECUTING` is
/// the single-use enforcement itself: a second call against the same row
/// sees a status the CAS no longer matches and fails.
#[allow(clippy::too_many_arguments)]
pub async fn authorize(
    handle: &RepositoryHandle,
    row_id: i64,
    target: &TargetIdentity,
    parameters: &Value,
    resource: &ResourceDescriptor,
    registry: &ActionTypeRegistry,
    context: &ActionContext,
    now: DateTime<Utc>,
) -> Result<PolicyApproved, PolicyError> {
    let row = get_action(handle, row_id)
        .await?
        .ok_or(PolicyError::NotFound(row_id))?;

    let supplied_hash = compute_parameters_hash(parameters)?;
    if supplied_hash != row.parameters_hash {
        return Err(PolicyError::ApprovalMismatch);
    }
    let supplied_target_json = serde_json::to_string(target)?;
    if supplied_target_json != row.target_identity_json {
        return Err(PolicyError::ApprovalMismatch);
    }
    let supplied_resource_json = serde_json::to_string(resource)?;
    if supplied_resource_json != row.resource_descriptor_json {
        return Err(PolicyError::ApprovalMismatch);
    }

    let current_status = row.status.as_str();
    if current_status != ActionStatus::AutoAllowed.as_str()
        && current_status != ActionStatus::Approved.as_str()
    {
        return Err(PolicyError::WrongStatus {
            id: row_id,
            expected: "APPROVED or AUTO_ALLOWED".to_string(),
            actual: current_status.to_string(),
        });
    }
    let check_not_expired = current_status == ActionStatus::Approved.as_str();

    let updated = transition_action(
        handle,
        row_id,
        current_status,
        ActionStatus::Executing.as_str(),
        now,
        check_not_expired,
        TransitionPatch::default(),
    )
    .await
    .map_err(map_transition_err)?;

    let entry = registry
        .lookup(&updated.action_type)
        .ok_or_else(|| PolicyError::UnknownActionType(updated.action_type.clone()))?;
    let request = ActionRequest {
        action_type: updated.action_type.clone(),
        target: *target,
        resource: resource.clone(),
        parameters: parameters.clone(),
        requested_by: updated.requested_by.clone(),
    };
    let constructed =
        (entry.construct)(&request, context).map_err(PolicyError::ConstructionFailed)?;
    let action: Arc<dyn crate::actions::ActionKind> = Arc::from(constructed);

    Ok(PolicyApproved::new(
        action,
        row_id,
        *target,
        resource.clone(),
    ))
}
