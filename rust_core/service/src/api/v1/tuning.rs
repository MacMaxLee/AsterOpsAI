//! Unit U14's tuning plan history view (the most recent plans, read
//! only), plus unit U23's real `POST /tuning/start`: `core::tuning::
//! start_plan` (already fully built and tested at the `core` level, unit
//! U10) wired for real. Starting a plan here genuinely runs the same
//! propose/evaluate pipeline unit U22's grant endpoint sits downstream
//! of. With only Low-risk action types registered and the real,
//! configurable `state.policy_environment` (unit U25) left at its
//! default `Development`, every candidate still resolves to
//! `AUTO_ALLOWED_PENDING`, not `PENDING_APPROVAL` — but with
//! `$ASTEROPS_POLICY_ENVIRONMENT=production` configured, `core::policy::
//! risk::decide(Low, Production)` genuinely requires approval, and the
//! full propose -> inbox -> grant -> execute loop ADR 0028 documented as
//! unreachable becomes real. See docs/adr/0028 and docs/adr/0030.

use ai_ops_core::policy::{ResourceDescriptor, ResourceKind, TargetIdentity};
use ai_ops_core::repository::{self, TuningPlanRow};
use ai_ops_core::tuning::{start_plan, AutomationMode, TuningPipeline, TuningProfile};
use axum::extract::{Extension, State};
use axum::Json;
use chrono::Utc;
use contracts::{ApiError, TuningCandidateOutcome, TuningPlanOutcome, TuningPlanSummary};
use serde::Deserialize;

use crate::actions::{target_verifier, tuning_error_to_api};
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

/// SRS FR-TUNE-001's four named profiles — `TuningProfile::Custom` isn't
/// offered here (unit U23): it needs its own `DesiredState` wire payload
/// (priority + affinity), a real, separable follow-on, not silently
/// half-supported.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WireTuningProfile {
    Balanced,
    HighPerformance,
    BatterySaver,
    Development,
}

impl From<WireTuningProfile> for TuningProfile {
    fn from(profile: WireTuningProfile) -> Self {
        match profile {
            WireTuningProfile::Balanced => TuningProfile::Balanced,
            WireTuningProfile::HighPerformance => TuningProfile::HighPerformance,
            WireTuningProfile::BatterySaver => TuningProfile::BatterySaver,
            WireTuningProfile::Development => TuningProfile::Development,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WireAutomationMode {
    RecommendOnly,
    AskBeforeChanges,
    AutoLowRisk,
}

impl From<WireAutomationMode> for AutomationMode {
    fn from(mode: WireAutomationMode) -> Self {
        match mode {
            WireAutomationMode::RecommendOnly => AutomationMode::RecommendOnly,
            WireAutomationMode::AskBeforeChanges => AutomationMode::AskBeforeChanges,
            WireAutomationMode::AutoLowRisk => AutomationMode::AutoLowRisk,
        }
    }
}

/// Process-only (unit U23): `core::tuning::build_candidates` itself
/// rejects a `DbSession` target (`TuningError::UnsupportedTarget`) — the
/// wire contract simply never offers one, rather than accepting a shape
/// that would always fail.
#[derive(Debug, Deserialize)]
pub struct StartPlanRequest {
    pub pid: u32,
    pub start_time_ticks: u64,
    pub resource_name: String,
    pub profile: WireTuningProfile,
    pub mode: WireAutomationMode,
    pub requested_by: String,
}

pub async fn start(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
    Json(body): Json<StartPlanRequest>,
) -> ApiResponse<TuningPlanOutcome> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "tuning plan history not available: repository layer did not start".to_string(),
            )
        })?;

        let target = TargetIdentity::Process {
            pid: body.pid,
            start_time_ticks: body.start_time_ticks,
        };
        let resource = ResourceDescriptor {
            kind: ResourceKind::Process,
            name: body.resource_name,
        };
        let pipeline = TuningPipeline {
            handle: &repo,
            registry: &state.action_registry,
            context: &state.action_context,
            verifier: &target_verifier(),
            protected: &state.protected_resources,
            // Real config as of unit U25 (`policy_config::
            // resolve_policy_environment`, `$ASTEROPS_POLICY_
            // ENVIRONMENT`) — defaults to `Development` when unset.
            environment: state.policy_environment,
        };

        let outcome = start_plan(
            target,
            resource,
            body.profile.into(),
            body.mode.into(),
            &body.requested_by,
            &pipeline,
            Utc::now(),
        )
        .await
        .map_err(tuning_error_to_api)?;

        Ok(TuningPlanOutcome {
            plan_id: outcome.plan_id,
            status: outcome.status.as_str().to_string(),
            candidates: outcome
                .candidates
                .into_iter()
                .map(|c| TuningCandidateOutcome {
                    action_type: c.action_type,
                    outcome: c.outcome,
                    row_id: c.row_id,
                    detail: c.detail,
                })
                .collect(),
        })
    }
    .await;

    ApiResponse::new(request_id, result)
}
