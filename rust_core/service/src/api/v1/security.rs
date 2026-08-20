//! Unit U15's security triage view: list open incidents, suppress a
//! noisy rule. No incident-close/dismiss endpoint — real, separate,
//! un-designed work (see docs/adr/0020). No detector-triggering
//! endpoint either: detectors are pure functions called from `core`'s
//! own callers, not something `service` invokes directly.

use ai_ops_core::policy::ResourceDescriptor;
use ai_ops_core::repository::{self, SecurityIncidentRow};
use ai_ops_core::security;
use axum::extract::{Extension, State};
use axum::Json;
use chrono::Utc;
use contracts::{ApiError, SecurityIncidentSummary};
use serde::Deserialize;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

fn to_summary((row, event_count): (SecurityIncidentRow, i64)) -> SecurityIncidentSummary {
    SecurityIncidentSummary {
        id: row.id,
        opened_at: row.opened_at,
        closed_at: row.closed_at,
        severity: row.severity,
        status: row.status,
        summary: row.summary,
        event_count,
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
