//! Unit U14's tuning plan history view: the most recent plans, read
//! only. No `POST` to start a plan — `core::tuning::start_plan` needs a
//! real `TuningPipeline` (registry, context, verifier, protected-resource
//! registry) `AppState` doesn't carry; that's real, separate work for a
//! future unit (see docs/adr/0019).

use ai_ops_core::repository::{self, TuningPlanRow};
use axum::extract::{Extension, State};
use contracts::{ApiError, TuningPlanSummary};

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

fn to_summary(row: &TuningPlanRow) -> TuningPlanSummary {
    TuningPlanSummary {
        id: row.id,
        created_at: row.created_at,
        target_identity_json: row.target_identity_json.clone(),
        profile: row.profile.clone(),
        mode: row.mode.clone(),
        status: row.status.clone(),
        completed_at: row.completed_at,
        candidates_json: row.candidates_json.clone(),
    }
}

pub async fn plans(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<Vec<TuningPlanSummary>> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "tuning plan history not available: repository layer did not start".to_string(),
            )
        })?;
        let rows = repository::list_recent_tuning_plans(&repo)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "list_recent_tuning_plans failed");
                ApiError::Internal
            })?;
        Ok(rows.iter().map(to_summary).collect())
    }
    .await;

    ApiResponse::new(request_id, result)
}
