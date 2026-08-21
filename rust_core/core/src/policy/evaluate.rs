//! Stage 2 of TRS §26's pipeline: policy evaluation. Computes the risk +
//! environment decision (SRS FR-POL-002), persists the lifecycle row, and
//! audits the outcome — including denials (SRS FR-POL-005 is explicit that
//! denials/expiries/rejections are audited too, not just approvals).

use chrono::{DateTime, Duration, Utc};

use crate::repository::{
    propose_action, record_audit_event, NewAuditEvent, NewProposedAction, RepositoryHandle,
};

use super::error::{map_propose_err, PolicyError};
use super::hash::compute_parameters_hash;
use super::registry::ActionTypeRegistry;
use super::request::ActionRequest;
use super::risk::{decide, Environment, PolicyDecision};
use super::status::ActionStatus;
use super::typestate::Validated;

/// TRS §25's default expiry.
pub const DEFAULT_APPROVAL_TTL_SECONDS: i64 = 300;

#[derive(Debug, Clone)]
pub enum PolicyOutcome {
    AutoAllowed {
        row_id: i64,
    },
    PendingApproval {
        row_id: i64,
        expires_at: DateTime<Utc>,
    },
    Denied {
        row_id: i64,
        reason: String,
    },
}

pub async fn evaluate(
    validated: Validated<ActionRequest>,
    environment: Environment,
    registry: &ActionTypeRegistry,
    handle: &RepositoryHandle,
    now: DateTime<Utc>,
) -> Result<PolicyOutcome, PolicyError> {
    let request = validated.into_inner();
    let entry = registry
        .lookup(&request.action_type)
        .ok_or_else(|| PolicyError::UnknownActionType(request.action_type.clone()))?;
    let risk = entry.risk_level;
    let decision = decide(risk, environment);

    let parameters_hash = compute_parameters_hash(&request.parameters)?;
    let target_identity_json = serde_json::to_string(&request.target)?;
    let resource_descriptor_json = serde_json::to_string(&request.resource)?;
    let parameters_json = serde_json::to_string(&request.parameters)?;

    let (status, approval_expires_at) = match decision {
        PolicyDecision::AutoAllow => (ActionStatus::AutoAllowed, None),
        PolicyDecision::RequireApproval => (
            ActionStatus::PendingApproval,
            Some(now + Duration::seconds(DEFAULT_APPROVAL_TTL_SECONDS)),
        ),
        PolicyDecision::Deny => (ActionStatus::Denied, None),
    };

    let new = NewProposedAction {
        created_at: now,
        action_type: request.action_type.clone(),
        target_identity_json,
        target_start_time: request.target.start_time_marker(),
        risk_classification: risk.as_str().to_string(),
        status: status.as_str().to_string(),
        evidence_json: "{}".to_string(),
        requested_by: request.requested_by.clone(),
        parameters_json,
        parameters_hash,
        resource_descriptor_json,
        approval_expires_at,
        rollback_of: None,
    };
    let row = propose_action(handle, new).await.map_err(map_propose_err)?;

    // FR-POL-005: every policy decision — including denials — is audited.
    record_audit_event(
        handle,
        NewAuditEvent {
            ts: now,
            event_type: format!("policy.{}", status.as_str().to_lowercase()),
            actor: request.requested_by.clone(),
            summary: format!(
                "{} action {} targeting {:?} {:?}",
                status.as_str(),
                request.action_type,
                entry.risk_level,
                request.resource.name
            ),
            detail_json: serde_json::to_string(&serde_json::json!({
                "row_id": row.id,
                "risk": risk.as_str(),
                "decision": format!("{decision:?}"),
            }))?,
        },
    )
    .await?;

    match decision {
        PolicyDecision::AutoAllow => Ok(PolicyOutcome::AutoAllowed { row_id: row.id }),
        PolicyDecision::RequireApproval => {
            let expires_at = approval_expires_at.unwrap_or(now);
            Ok(PolicyOutcome::PendingApproval {
                row_id: row.id,
                expires_at,
            })
        }
        PolicyDecision::Deny => Ok(PolicyOutcome::Denied {
            row_id: row.id,
            reason: format!("risk level {} denied in {environment:?}", risk.as_str()),
        }),
    }
}
