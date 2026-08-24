//! Unit U15's security triage view: list open incidents, suppress a
//! noisy rule. Unit U76 (ADR 0016/0020's own named gap, resolved to
//! manual-only — docs/adr/0081) adds a real incident-close endpoint.
//! No detector-triggering endpoint: detectors are pure functions
//! called from `core`'s own callers, not something `service` invokes
//! directly.

use ai_ops_core::policy::ResourceDescriptor;
use ai_ops_core::repository::{self, IncidentDetail, NewAuditEvent};
use ai_ops_core::security;
use axum::extract::{Extension, Path, State};
use axum::Json;
use chrono::Utc;
use contracts::{ApiError, SecurityIncidentSummary};
use serde::Deserialize;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

fn to_summary(detail: IncidentDetail) -> SecurityIncidentSummary {
    SecurityIncidentSummary {
        id: detail.incident.id,
        opened_at: detail.incident.opened_at,
        closed_at: detail.incident.closed_at,
        severity: detail.incident.severity,
        status: detail.incident.status,
        summary: detail.incident.summary,
        event_count: detail.event_count,
        detector_id: detail.detector_id,
        resource_kind: detail.resource_kind,
        resource_name: detail.resource_name,
    }
}

pub async fn incidents(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<SecurityIncidentSummary>> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "security incidents not available: repository layer did not start".to_string(),
            )
        })?;
        let rows = repository::list_open_security_incidents(&repo)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "list_open_security_incidents failed");
                ApiError::Internal
            })?;
        Ok(rows.into_iter().map(to_summary).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}

#[derive(Debug, Deserialize)]
pub struct CloseIncidentRequest {
    pub closed_by: String,
}

/// Unit U76: `POST /security/incidents/{id}/close` — manual-only, per
/// ADR 0081 (ADR 0016/0020 explicitly left "what closes an incident"
/// un-designed rather than guess between auto-resolution and a manual
/// operator action). Idempotent: closing an already-`CLOSED` incident
/// re-returns it unchanged rather than erroring. `security_incidents`
/// has no `closed_by` column (unlike suppression's own `created_by`),
/// so who closed it is recorded the same way `policy::grant`'s own
/// auto-allowed path records its actor — a real `audit_events` row,
/// not a schema change for this alone.
pub async fn close(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Path(id): Path<i64>,
    Json(body): Json<CloseIncidentRequest>,
) -> ApiResponse<SecurityIncidentSummary> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "security incidents not available: repository layer did not start".to_string(),
            )
        })?;
        let now = Utc::now();
        let closed = repository::close_security_incident(&repo, id, now)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, incident_id = id, "close_security_incident failed");
                ApiError::Internal
            })?
            .ok_or(ApiError::NotFound)?;

        repository::record_audit_event(
            &repo,
            NewAuditEvent {
                ts: now,
                event_type: "security.incident_closed".to_string(),
                actor: body.closed_by,
                summary: format!("security incident {id} closed"),
                detail_json: serde_json::json!({ "incident_id": id }).to_string(),
            },
        )
        .await
        .map_err(|err| {
            tracing::error!(error = %err, incident_id = id, "record_audit_event failed for incident close");
            ApiError::Internal
        })?;

        Ok(to_summary(closed))
    }
    .await;

    ApiResponse::new(request_id, result)
}

#[derive(Debug, Deserialize)]
pub struct SuppressRequest {
    pub detector_id: String,
    pub resource: Option<ResourceDescriptor>,
    pub reason: String,
    pub created_by: String,
}

pub async fn suppress(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Json(body): Json<SuppressRequest>,
) -> ApiResponse<()> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "security incidents not available: repository layer did not start".to_string(),
            )
        })?;
        security::suppress(
            &repo,
            &body.detector_id,
            body.resource.as_ref(),
            &body.reason,
            &body.created_by,
            Utc::now(),
        )
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "security::suppress failed");
            ApiError::Internal
        })
    }
    .await;

    ApiResponse::new(request_id, result)
}
