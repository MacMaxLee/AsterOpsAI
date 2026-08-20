//! Rollback (TRS §27 / SRS FR-BENCH-003): re-verifies target identity
//! immediately before acting, same as the executor's own apply stage, then
//! inserts a new lifecycle row with `rollback_of` set to the original
//! action's id — a rollback is itself an action attempt, not a mutation of
//! the original row, so the original's own final status/result stay
//! exactly as executed left them.

use chrono::Utc;

use crate::policy::ActionStatus;
use crate::repository::{
    propose_action, record_audit_event, transition_action, NewAuditEvent, NewProposedAction,
    RepositoryHandle, TransitionPatch,
};

use super::error::ActionError;
use super::executor::Executed;
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
