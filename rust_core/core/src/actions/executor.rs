//! TRS §26 stages 4-9, the executor half of the fixed pipeline (stages 1-3
//! are `core::policy`'s job, producing the `PolicyApproved` this module's
//! only public entry point accepts). Each stage is independently
//! unit-testable and none may be skipped — `execute` runs them in this
//! exact order, returning as soon as any stage fails.

use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;

use crate::policy::{
    ActionStatus, PolicyApproved, ProtectedResourceRegistry, ResourceDescriptor, TargetIdentity,
};
use crate::repository::{
    record_audit_event, transition_action, NewAuditEvent, RepositoryHandle, TransitionPatch,
};

use super::error::ActionError;
use super::kind::ActionKind;
use super::target_verifier::TargetVerifier;

/// TRS §24's final typestate: `PolicyApproved -> Executed`.
pub struct Executed {
    row_id: i64,
    target: TargetIdentity,
    resource: ResourceDescriptor,
    previous_state: Value,
    action: Arc<dyn ActionKind>,
}

impl Executed {
    pub fn row_id(&self) -> i64 {
        self.row_id
    }

    pub fn target(&self) -> &TargetIdentity {
        &self.target
    }

    pub fn resource(&self) -> &ResourceDescriptor {
        &self.resource
    }

    pub fn previous_state(&self) -> &Value {
        &self.previous_state
    }

    pub fn action(&self) -> &dyn ActionKind {
        self.action.as_ref()
    }
}

async fn fail(handle: &RepositoryHandle, row_id: i64, err: &ActionError) {
    let now = Utc::now();
    let patch = TransitionPatch {
        result_json: Some(serde_json::json!({ "error": err.to_string() }).to_string()),
        ..Default::default()
    };
    // Best-effort: if even the failure-recording write fails, the error
    // that caused this path is still returned to the caller — we don't
    // want a secondary logging failure to mask the real one.
    if let Err(write_err) = transition_action(
        handle,
        row_id,
        ActionStatus::Executing.as_str(),
        ActionStatus::Failed.as_str(),
        now,
        false,
        patch,
    )
    .await
    {
        tracing::warn!(error = %write_err, row_id, "failed to record action failure");
    }
    let _ = record_audit_event(
        handle,
        NewAuditEvent {
            ts: now,
            event_type: "policy.execution_failed".to_string(),
            actor: "system".to_string(),
            summary: format!("action {row_id} failed: {err}"),
            detail_json: serde_json::json!({ "row_id": row_id, "error": err.to_string() })
                .to_string(),
        },
    )
    .await;
}

/// The executor's only public entry point — it accepts a `PolicyApproved`
/// and nothing else (TRS §24). Stages 4-9, in order: capability check,
/// protected-resource check, capture previous state, **re-verify target
/// identity** then apply, verify, audit.
pub async fn execute(
    approved: PolicyApproved,
    verifier: &dyn TargetVerifier,
    protected: &ProtectedResourceRegistry,
    handle: &RepositoryHandle,
) -> Result<Executed, ActionError> {
    let row_id = approved.row_id();
    let target = *approved.target();
    let resource = approved.resource().clone();
    let action = approved.action_arc();

    // Stage 4: capability check.
    if let Err(err) = action.check_capability() {
        fail(handle, row_id, &err).await;
        return Err(err);
    }

    // Stage 5: protected-resource check.
    if let Err(reason) = protected.check(&resource) {
        let err = ActionError::Protected(reason);
        fail(handle, row_id, &err).await;
        return Err(err);
    }

    // Stage 6: capture previous state.
    let previous_state = match action.capture_previous_state() {
        Ok(state) => state,
        Err(err) => {
            fail(handle, row_id, &err).await;
            return Err(err);
        }
    };

    // Stage 7: re-verify target identity immediately before apply (TRS
    // §27), then apply.
    if let Err(err) = verifier.verify(&target) {
        fail(handle, row_id, &err).await;
        return Err(err);
    }
    if let Err(err) = action.apply() {
        fail(handle, row_id, &err).await;
        return Err(err);
    }

    // Stage 8: verify.
    if let Err(err) = action.verify() {
        fail(handle, row_id, &err).await;
        return Err(err);
    }

    // Stage 9: audit — record success.
    let now = Utc::now();
    let result_json = serde_json::json!({ "status": "ok" }).to_string();
    transition_action(
        handle,
        row_id,
        ActionStatus::Executing.as_str(),
        ActionStatus::Executed.as_str(),
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
            event_type: "policy.executed".to_string(),
            actor: "system".to_string(),
            summary: format!("action {row_id} ({}) executed", action.action_type()),
            detail_json: serde_json::json!({ "row_id": row_id }).to_string(),
        },
    )
    .await?;

    Ok(Executed {
        row_id,
        target,
        resource,
        previous_state,
        action,
    })
}
