//! Rollback (TRS §27 / SRS FR-BENCH-003): re-verifies target identity
//! immediately before acting, same as the executor's own apply stage, then
//! inserts a new lifecycle row with `rollback_of` set to the original
//! action's id — a rollback is itself an action attempt, not a mutation of
//! the original row, so the original's own final status/result stay
//! exactly as executed left them.

use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;

use crate::policy::{
    ActionRequest, ActionStatus, ActionTypeRegistry, PolicyError, ResourceDescriptor,
    TargetIdentity,
};
use crate::repository::{
    get_action, propose_action, record_audit_event, transition_action, NewAuditEvent,
    NewProposedAction, RepositoryHandle, TransitionPatch,
};

use super::context::ActionContext;
use super::error::ActionError;
use super::executor::Executed;
use super::kind::ActionKind;
use super::target_verifier::TargetVerifier;

/// Returns the new rollback row's id on success (whether or not the
/// underlying `rollback()` call itself succeeded — the attempt is always
/// recorded; only a genuine re-verification failure prevents even
/// attempting it).
pub async fn rollback(
    executed: &Executed,
    verifier: &dyn TargetVerifier,
    rolled_back_by: impl Into<String>,
    handle: &RepositoryHandle,
) -> Result<i64, ActionError> {
    // TRS §27 / FR-BENCH-003: re-verified immediately before rollback too,
    // not just before the original apply.
    verifier.verify(executed.target())?;

    let now = Utc::now();
    let target_identity_json = serde_json::to_string(executed.target())?;
    let resource_descriptor_json = serde_json::to_string(executed.resource())?;

    let new = NewProposedAction {
        created_at: now,
        action_type: executed.action().action_type().to_string(),
        target_identity_json,
        target_start_time: executed.target().start_time_marker(),
        risk_classification: executed.action().risk_level().as_str().to_string(),
        status: ActionStatus::Executing.as_str().to_string(),
        evidence_json: "{}".to_string(),
        requested_by: rolled_back_by.into(),
        parameters_json: "{}".to_string(),
        parameters_hash: String::new(),
        resource_descriptor_json,
        approval_expires_at: None,
        rollback_of: Some(executed.row_id()),
    };
    let row = propose_action(handle, new).await?;

    let result = executed.action().rollback(executed.previous_state());
    let (status, result_json) = match &result {
        Ok(()) => (
            ActionStatus::RolledBack,
            serde_json::json!({"status": "ok"}).to_string(),
        ),
        Err(err) => (
            ActionStatus::Failed,
            serde_json::json!({"error": err.to_string()}).to_string(),
        ),
    };

    transition_action(
        handle,
        row.id,
        ActionStatus::Executing.as_str(),
        status.as_str(),
        now,
        false,
        TransitionPatch {
            executed_at: Some(now),
            result_json: Some(result_json),
            ..Default::default()
        },
    )
    .await?;

    record_audit_event(
        handle,
        NewAuditEvent {
            ts: now,
            event_type: format!("policy.{}", status.as_str().to_lowercase()),
            actor: "system".to_string(),
            summary: format!(
                "rollback of action {} -> {}",
                executed.row_id(),
                status.as_str()
            ),
            detail_json: serde_json::json!({
                "row_id": row.id,
                "rollback_of": executed.row_id(),
            })
            .to_string(),
        },
    )
    .await?;

    result?;
    Ok(row.id)
}

/// Unit U28: the only way to reach `rollback()` for a row that wasn't
/// just executed in the same call chain — real, urgent gap found after
/// unit U27 wired `security.suspend_process`: nothing outside this
/// crate could construct the `Executed` `rollback()` requires, so
/// nothing could ever resume a process suspended through the running
/// product. `previous_state` was never persisted (no DB column for it),
/// so this always reconstructs it as `Value::Null` — genuinely correct
/// for `security.suspend_process` (its own `rollback()` ignores
/// `previous_state` entirely, unconditional `resume_process`), and a
/// real, honest, *safe* failure for `host.set_process_priority`/`host.
/// set_process_cpu_affinity` (both their own `rollback()`s reject
/// anything that isn't the specific shape they expect — never a wrong
/// or silent rollback). See docs/adr/0033.
pub async fn rollback_by_row_id(
    row_id: i64,
    registry: &ActionTypeRegistry,
    context: &ActionContext,
    verifier: &dyn TargetVerifier,
    rolled_back_by: impl Into<String>,
    handle: &RepositoryHandle,
) -> Result<i64, ActionError> {
    let row = get_action(handle, row_id)
        .await?
        .ok_or(PolicyError::NotFound(row_id))?;

    if row.status != "EXECUTED" {
        return Err(ActionError::Policy(PolicyError::WrongStatus {
            id: row_id,
            expected: "EXECUTED".to_string(),
            actual: row.status,
        }));
    }

    let entry = registry
        .lookup(&row.action_type)
        .ok_or_else(|| PolicyError::UnknownActionType(row.action_type.clone()))?;
    // Unit U10's own `reversible` flag: "proven to round-trip without
    // any privilege gap" (its own doc comment, `core::policy::registry`)
    // — already used to gate `AUTO_LOW_RISK`'s auto-execute eligibility;
    // reused here as a second, independent pre-flight guard, refusing
    // outright rather than attempting a rollback `rollback()` itself
    // has no opinion on whether the caller should have tried.
    if !entry.reversible {
        return Err(ActionError::CapabilityUnavailable(format!(
            "action type {} is not reversible",
            row.action_type
        )));
    }

    let target: TargetIdentity = serde_json::from_str(&row.target_identity_json)?;
    let resource: ResourceDescriptor = serde_json::from_str(&row.resource_descriptor_json)?;
    let parameters: Value = serde_json::from_str(&row.parameters_json)?;
    let request = ActionRequest {
        action_type: row.action_type.clone(),
        target,
        resource: resource.clone(),
        parameters,
        requested_by: row.requested_by.clone(),
    };
    let constructed =
        (entry.construct)(&request, context).map_err(PolicyError::ConstructionFailed)?;
    let action: Arc<dyn ActionKind> = Arc::from(constructed);

    let executed = Executed::reconstruct(row_id, target, resource, Value::Null, action);
    rollback(&executed, verifier, rolled_back_by, handle).await
}
