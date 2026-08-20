//! `start_plan` — the orchestrator. Inserts the `IN_FLIGHT` row
//! (FR-TUNE-002's real concurrency guard lives in
//! `repository::tuning::insert_tuning_plan`, not here), builds candidates,
//! then processes each candidate according to `AutomationMode`, and marks
//! the plan `COMPLETED`/`REJECTED`.
//!
//! Deliberately takes no Linux-only dependency directly: the real
//! `TargetVerifier`/`ProtectedResourceRegistry` a caller needs for
//! `AutoLowRisk` to actually execute anything are supplied via
//! [`TuningPipeline`], the same "unmodified U7/U8 pipeline" every other
//! caller of `policy::evaluate`/`actions::execute` already uses — this
//! module adds no second way to reach `apply()`.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::actions::{execute, ActionContext, TargetVerifier};
use crate::policy::{
    approval, evaluate, validate, ActionRequest, ActionTypeRegistry, Environment, PolicyOutcome,
    ProtectedResourceRegistry, ResourceDescriptor, TargetIdentity,
};
use crate::repository::{
    get_tuning_plan, insert_tuning_plan, mark_tuning_plan_completed, NewTuningPlan,
    RepositoryHandle, TuningPlanRow,
};

use super::candidates::build_candidates;
use super::error::TuningError;
use super::history::has_improved_history;
use super::mode::{mode_label, AutomationMode};
use super::profile::{desired_state_for, full_cpu_set, profile_label, TuningProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuningPlanStatus {
    InFlight,
    Completed,
    Rejected,
}

impl TuningPlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InFlight => "IN_FLIGHT",
            Self::Completed => "COMPLETED",
            Self::Rejected => "REJECTED",
        }
    }
}

/// One candidate's final disposition, as recorded in the plan row's
/// `candidates_json` (and returned to the caller). `row_id` is the
/// `actions` row when one was ever proposed (every outcome except
/// `RECOMMENDED`/a pre-proposal failure).
#[derive(Debug, Clone, Serialize)]
pub struct CandidateOutcome {
    pub action_type: String,
    pub outcome: String,
    pub row_id: Option<i64>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TuningOutcome {
    pub plan_id: i64,
    pub status: TuningPlanStatus,
    pub candidates: Vec<CandidateOutcome>,
}

/// The real, shared dependencies `policy::evaluate`/`policy::approval::
/// authorize`/`actions::execute` need — bundled so `start_plan` itself
/// stays free of any platform-specific import (see this module's own doc
/// comment).
pub struct TuningPipeline<'a> {
    pub handle: &'a RepositoryHandle,
    pub registry: &'a ActionTypeRegistry,
    pub context: &'a ActionContext,
    pub verifier: &'a dyn TargetVerifier,
    pub protected: &'a ProtectedResourceRegistry,
    pub environment: Environment,
}

async fn reject_plan(handle: &RepositoryHandle, id: i64, now: DateTime<Utc>, err: &TuningError) {
    let detail = serde_json::json!([{ "error": err.to_string() }]).to_string();
    if let Err(write_err) =
        mark_tuning_plan_completed(handle, id, TuningPlanStatus::Rejected.as_str(), now, detail)
            .await
    {
        tracing::warn!(error = %write_err, plan_id = id, "failed to record rejected tuning plan");
    }
}

async fn process_candidate(
    request: ActionRequest,
    mode: AutomationMode,
    pipeline: &TuningPipeline<'_>,
    now: DateTime<Utc>,
) -> CandidateOutcome {
    let action_type = request.action_type.clone();

    if matches!(mode, AutomationMode::RecommendOnly) {
        return CandidateOutcome {
            action_type,
            outcome: "RECOMMENDED".to_string(),
            row_id: None,
            detail: None,
        };
    }

    let validated = match validate(request.clone(), pipeline.registry) {
        Ok(v) => v,
        Err(err) => {
            return CandidateOutcome {
                action_type,
                outcome: "VALIDATION_FAILED".to_string(),
                row_id: None,
                detail: Some(err.to_string()),
            }
        }
    };

    let policy_outcome = match evaluate(
        validated,
        pipeline.environment,
        pipeline.registry,
        pipeline.handle,
        now,
    )
    .await
    {
        Ok(o) => o,
        Err(err) => {
            return CandidateOutcome {
                action_type,
                outcome: "EVALUATION_FAILED".to_string(),
                row_id: None,
                detail: Some(err.to_string()),
            }
        }
    };

    let row_id = match policy_outcome {
        PolicyOutcome::Denied { row_id, reason } => {
            return CandidateOutcome {
                action_type,
                outcome: "DENIED".to_string(),
                row_id: Some(row_id),
                detail: Some(reason),
            }
        }
        PolicyOutcome::PendingApproval { row_id, .. } => {
            return CandidateOutcome {
                action_type,
                outcome: "PENDING_APPROVAL".to_string(),
                row_id: Some(row_id),
                detail: None,
            }
        }
        PolicyOutcome::AutoAllowed { row_id } => row_id,
    };

    // FR-TUNE-003's three-part AUTO_LOW_RISK gate: AutoAllowed (checked
    // above), reversible, and a recorded improved-outcome history. A
    // history-query failure defaults to "no history" — never silently
    // auto-execute on an error, the same conservative-default reasoning
    // `ActionTypeEntry.reversible`'s own `false` default uses.
    let should_auto_execute = matches!(mode, AutomationMode::AutoLowRisk) && {
        let reversible = pipeline
            .registry
            .lookup(&action_type)
            .map(|entry| entry.reversible)
            .unwrap_or(false);
        reversible
            && has_improved_history(pipeline.handle, &action_type)
                .await
                .unwrap_or(false)
    };

    if !should_auto_execute {
        return CandidateOutcome {
            action_type,
            outcome: "AUTO_ALLOWED_PENDING".to_string(),
            row_id: Some(row_id),
            detail: None,
        };
    }

    let approved = match approval::authorize(
        pipeline.handle,
        row_id,
        &request.target,
        &request.parameters,
        &request.resource,
        pipeline.registry,
        pipeline.context,
        now,
    )
    .await
    {
        Ok(a) => a,
        Err(err) => {
            return CandidateOutcome {
                action_type,
                outcome: "AUTHORIZATION_FAILED".to_string(),
                row_id: Some(row_id),
                detail: Some(err.to_string()),
            }
        }
    };

    match execute(
        approved,
        pipeline.verifier,
        pipeline.protected,
        pipeline.handle,
    )
    .await
    {
        Ok(_) => CandidateOutcome {
            action_type,
            outcome: "AUTO_EXECUTED".to_string(),
            row_id: Some(row_id),
            detail: None,
        },
        Err(err) => CandidateOutcome {
            action_type,
            outcome: "EXECUTION_FAILED".to_string(),
            row_id: Some(row_id),
            detail: Some(err.to_string()),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn start_plan(
    target: TargetIdentity,
    resource: ResourceDescriptor,
    profile: TuningProfile,
    mode: AutomationMode,
    requested_by: &str,
    pipeline: &TuningPipeline<'_>,
    now: DateTime<Utc>,
) -> Result<TuningOutcome, TuningError> {
    let target_identity_json = serde_json::to_string(&target)?;
    let profile_str = profile_label(&profile).to_string();

    let plan_row: TuningPlanRow = insert_tuning_plan(
        pipeline.handle,
        NewTuningPlan {
            created_at: now,
            target_identity_json,
            target_start_time: target.start_time_marker(),
            profile: profile_str,
            mode: mode_label(mode).to_string(),
            status: TuningPlanStatus::InFlight.as_str().to_string(),
            candidates_json: "[]".to_string(),
        },
    )
    .await?;

    let desired = desired_state_for(&profile, &full_cpu_set());
    let candidates =
        match build_candidates(&desired, target, &resource, pipeline.context, requested_by) {
            Ok(c) => c,
            Err(err) => {
                reject_plan(pipeline.handle, plan_row.id, now, &err).await;
                return Err(err);
            }
        };

    let mut outcomes = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        outcomes.push(process_candidate(candidate, mode, pipeline, now).await);
    }

    let candidates_json = serde_json::to_string(&outcomes)?;
    mark_tuning_plan_completed(
        pipeline.handle,
        plan_row.id,
        TuningPlanStatus::Completed.as_str(),
        now,
        candidates_json,
    )
    .await?;

    Ok(TuningOutcome {
        plan_id: plan_row.id,
        status: TuningPlanStatus::Completed,
        candidates: outcomes,
    })
}

pub async fn get_plan(
    handle: &RepositoryHandle,
    id: i64,
) -> Result<Option<TuningPlanRow>, TuningError> {
    get_tuning_plan(handle, id).await.map_err(Into::into)
}
