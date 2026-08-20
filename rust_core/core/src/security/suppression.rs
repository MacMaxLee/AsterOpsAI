//! SRS FR-SEC-005: manageable per-rule and per-resource false positives,
//! with a recorded audit trail. `suppress` writes the suppression row
//! *and* a real `audit_events` row through the existing chain (ADR
//! 0002's tamper-evident log) — no second, parallel audit mechanism.

use chrono::{DateTime, Utc};

use crate::policy::ResourceDescriptor;
use crate::repository::{
    record_audit_event, record_security_suppression, NewAuditEvent, NewSecuritySuppression,
    RepositoryHandle,
};

use super::error::SecurityError;

/// `resource: None` suppresses `detector_id` everywhere; `Some` suppresses
/// it only for that exact resource.
pub async fn suppress(
    handle: &RepositoryHandle,
    detector_id: &str,
    resource: Option<&ResourceDescriptor>,
    reason: &str,
    created_by: &str,
    now: DateTime<Utc>,
) -> Result<(), SecurityError> {
    let resource_descriptor_json = resource.map(serde_json::to_string).transpose()?;

    record_security_suppression(
        handle,
        NewSecuritySuppression {
            created_at: now,
            created_by: created_by.to_string(),
            detector_id: detector_id.to_string(),
            resource_descriptor_json: resource_descriptor_json.clone(),
            reason: reason.to_string(),
        },
    )
    .await?;

    record_audit_event(
        handle,
        NewAuditEvent {
            ts: now,
            event_type: "security.suppressed".to_string(),
            actor: created_by.to_string(),
            summary: format!("detector '{detector_id}' suppressed: {reason}"),
            detail_json: serde_json::json!({
                "detector_id": detector_id,
                "resource_descriptor_json": resource_descriptor_json,
                "reason": reason,
            })
            .to_string(),
        },
    )
    .await?;

    Ok(())
}
