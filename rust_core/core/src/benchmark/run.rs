//! The benchmark orchestrator: collect a baseline window, gate on
//! stability (SRS FR-BENCH-001, TRS §34), apply the action through the
//! real, unmodified U7/U8 pipeline, collect an identical-shape post-change
//! window, resolve a verdict, and — per TRS §35's rollback triggers —
//! roll back on `Regressed`/`Unstable`.

use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::actions::{execute, rollback as rollback_action, ActionContext, TargetVerifier};
use crate::policy::{
    approval, evaluate, validate, ActionRequest, ActionTypeRegistry, Environment,
    ProtectedResourceRegistry,
};
use crate::repository::{
    insert_benchmark_run, mark_benchmark_run_rolled_back, record_audit_event, NewAuditEvent,
    NewBenchmarkRun, RepositoryHandle,
};

use super::config::BenchmarkConfig;
use super::confounders::{Confounders, SampleGap};
use super::error::BenchmarkError;
use super::sampler::{MetricDirection, MetricSampler};
use super::verdict::{resolve, BenchmarkVerdict, BASELINE_UNSTABLE};

pub struct BenchmarkRunRequest<'a> {
    pub action_request: ActionRequest,
    pub metric_name: String,
    pub sampler: &'a dyn MetricSampler,
    pub direction: MetricDirection,
    pub config: BenchmarkConfig,
}

/// The collaborators `run_benchmark` needs from U7/U8's pipeline —
/// bundled into one struct rather than another `#[allow(too_many_arguments)]`
/// function.
pub struct BenchmarkPipeline<'a> {
    pub registry: &'a ActionTypeRegistry,
    pub environment: Environment,
    pub context: &'a ActionContext,
    pub verifier: &'a dyn TargetVerifier,
    pub protected: &'a ProtectedResourceRegistry,
    pub handle: &'a RepositoryHandle,
}

#[derive(Debug, Clone)]
pub enum BenchmarkOutcome {
    /// TRS §34: baseline coefficient of variation exceeded the stability
    /// threshold — the action was never applied.
    BaselineAborted {
        run_id: i64,
        baseline_cv: Option<f64>,
    },
    Completed {
        run_id: i64,
        action_row_id: i64,
        verdict: BenchmarkVerdict,
        rolled_back: bool,
        rollback_action_row_id: Option<i64>,
    },
}

struct Window {
    samples: Vec<f64>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    gaps: Vec<SampleGap>,
}

async fn collect_window(
    sampler: &dyn MetricSampler,
    config: &BenchmarkConfig,
) -> Result<Window, BenchmarkError> {
    let mut samples = Vec::new();
    let mut timestamps = Vec::new();
    let start = Utc::now();
    loop {
        let value = sampler.sample()?;
        let ts = Utc::now();
        samples.push(value);
        timestamps.push(ts);
        let elapsed = (ts - start).to_std().unwrap_or(Duration::ZERO);
        if elapsed >= config.min_window && samples.len() >= config.min_samples {
            break;
        }
        tokio::time::sleep(config.poll_interval).await;
    }
    let end = *timestamps.last().unwrap_or(&start);
    let gaps = detect_sample_gaps(&timestamps, config.poll_interval);
    Ok(Window {
        samples,
        start,
        end,
        gaps,
    })
}

fn detect_sample_gaps(timestamps: &[DateTime<Utc>], poll_interval: Duration) -> Vec<SampleGap> {
    let threshold_seconds = poll_interval.as_secs_f64() * 2.0;
    let mut gaps = Vec::new();
    for pair in timestamps.windows(2) {
        let gap_seconds = (pair[1] - pair[0]).num_milliseconds() as f64 / 1000.0;
        if gap_seconds > threshold_seconds {
            gaps.push(SampleGap {
                after: pair[0],
                before: pair[1],
                gap_seconds,
            });
        }
    }
    gaps
}

#[cfg(target_os = "linux")]
fn read_process_count() -> Option<i64> {
    std::fs::read_dir("/proc").ok().map(|entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty())
            })
            .count() as i64
    })
}

#[cfg(not(target_os = "linux"))]
fn read_process_count() -> Option<i64> {
    None
}

pub async fn run_benchmark(
    request: BenchmarkRunRequest<'_>,
    pipeline: &BenchmarkPipeline<'_>,
) -> Result<BenchmarkOutcome, BenchmarkError> {
    let process_count_before = read_process_count();

    let baseline = collect_window(request.sampler, &request.config).await?;
    if baseline.samples.len() < request.config.min_samples {
        return Err(BenchmarkError::InsufficientSamples {
            got: baseline.samples.len(),
            need: request.config.min_samples,
        });
    }

    let baseline_cv = super::stats::coefficient_of_variation(&baseline.samples);
    let now = Utc::now();

    if baseline_cv.is_some_and(|cv| cv >= request.config.stability_cv_threshold) {
        let confounders = Confounders {
            process_count_delta: None,
            sample_gaps: baseline.gaps.clone(),
            ..Confounders::default()
        };
        let run = insert_benchmark_run(
            pipeline.handle,
            NewBenchmarkRun {
                created_at: now,
                action_id: None,
                metric_name: request.metric_name.clone(),
                direction: direction_str(request.direction).to_string(),
                baseline_window_start: baseline.start,
                baseline_window_end: baseline.end,
                baseline_cv,
                post_change_window_start: None,
                post_change_window_end: None,
                post_change_cv: None,
                p_value: None,
                verdict: BASELINE_UNSTABLE.to_string(),
                confounders_json: serde_json::to_string(&confounders)?,
            },
        )
        .await?;
        record_audit_event(
            pipeline.handle,
            NewAuditEvent {
                ts: now,
                event_type: "benchmark.baseline_unstable".to_string(),
                actor: "system".to_string(),
                summary: format!(
                    "benchmark of {} aborted: baseline coefficient of variation too high",
                    request.metric_name
                ),
                detail_json: serde_json::json!({ "run_id": run.id, "baseline_cv": baseline_cv })
                    .to_string(),
            },
        )
        .await?;
        return Ok(BenchmarkOutcome::BaselineAborted {
            run_id: run.id,
            baseline_cv,
        });
    }

    // Real U7/U8 pipeline, unmodified.
    let validated = validate(request.action_request.clone(), pipeline.registry)?;
    let outcome = evaluate(
        validated,
        pipeline.environment,
        pipeline.registry,
        pipeline.handle,
        now,
    )
    .await?;
    let action_row_id = match outcome {
        crate::policy::PolicyOutcome::AutoAllowed { row_id } => row_id,
        crate::policy::PolicyOutcome::PendingApproval { row_id, .. } => {
            return Err(BenchmarkError::Policy(crate::policy::PolicyError::Denied(
                format!(
                    "action {row_id} requires approval — a benchmark run only proceeds on \
                     auto-allowed (low-risk) actions"
                ),
            )));
        }
        crate::policy::PolicyOutcome::Denied { reason, .. } => {
            return Err(BenchmarkError::Policy(crate::policy::PolicyError::Denied(
                reason,
            )));
        }
    };

    let approved = approval::authorize(
        pipeline.handle,
        action_row_id,
        &request.action_request.target,
        &request.action_request.parameters,
        &request.action_request.resource,
        pipeline.registry,
        pipeline.context,
        now,
    )
    .await?;

    let executed = execute(
        approved,
        pipeline.verifier,
        pipeline.protected,
        pipeline.handle,
    )
    .await?;

    let post_change = collect_window(request.sampler, &request.config).await?;
    let post_change_cv = super::stats::coefficient_of_variation(&post_change.samples);
    let mann_whitney = super::stats::mann_whitney_u(&baseline.samples, &post_change.samples);

    let verdict = resolve(
        &baseline.samples,
        &post_change.samples,
        request.direction,
        &request.config,
    );

    let process_count_after = read_process_count();
    let confounders = Confounders {
        process_count_delta: match (process_count_before, process_count_after) {
            (Some(before), Some(after)) => Some(after - before),
            _ => None,
        },
        sample_gaps: baseline
            .gaps
            .iter()
            .chain(post_change.gaps.iter())
            .cloned()
            .collect(),
        ..Confounders::default()
    };

    let run = insert_benchmark_run(
        pipeline.handle,
        NewBenchmarkRun {
            created_at: now,
            action_id: Some(action_row_id),
            metric_name: request.metric_name.clone(),
            direction: direction_str(request.direction).to_string(),
            baseline_window_start: baseline.start,
            baseline_window_end: baseline.end,
            baseline_cv,
            post_change_window_start: Some(post_change.start),
            post_change_window_end: Some(post_change.end),
            post_change_cv,
            p_value: mann_whitney.map(|r| r.p_value),
            verdict: verdict.as_str().to_string(),
            confounders_json: serde_json::to_string(&confounders)?,
        },
    )
    .await?;

    // TRS §35: REGRESSED/UNSTABLE are rollback triggers.
    let mut rolled_back = false;
    let mut rollback_action_row_id = None;
    if matches!(
        verdict,
        BenchmarkVerdict::Regressed | BenchmarkVerdict::Unstable
    ) {
        match rollback_action(
            &executed,
            pipeline.verifier,
            "benchmark-system",
            pipeline.handle,
        )
        .await
        {
            Ok(rollback_id) => {
                mark_benchmark_run_rolled_back(pipeline.handle, run.id, rollback_id).await?;
                rolled_back = true;
                rollback_action_row_id = Some(rollback_id);
            }
            Err(err) => {
                // TRS §35: "a failed rollback is a CRITICAL audit event
                // surfaced in the UI, not merely logged" — no structured
                // severity field exists on NewAuditEvent (U2), so severity
                // is encoded in event_type instead (see docs/adr/0014).
                record_audit_event(
                    pipeline.handle,
                    NewAuditEvent {
                        ts: Utc::now(),
                        event_type: "benchmark.rollback_failed".to_string(),
                        actor: "system".to_string(),
                        summary: format!(
                            "CRITICAL: rollback of action {action_row_id} (benchmark run {}) failed: {err}",
                            run.id
                        ),
                        detail_json: serde_json::json!({
                            "run_id": run.id,
                            "action_id": action_row_id,
                            "error": err.to_string(),
                        })
                        .to_string(),
                    },
                )
                .await?;
            }
        }
    }

    record_audit_event(
        pipeline.handle,
        NewAuditEvent {
            ts: Utc::now(),
            event_type: format!("benchmark.{}", verdict.as_str().to_lowercase()),
            actor: "system".to_string(),
            summary: format!(
                "benchmark of {} on action {action_row_id}: {}",
                request.metric_name,
                verdict.as_str()
            ),
            detail_json: serde_json::json!({
                "run_id": run.id,
                "action_id": action_row_id,
                "rolled_back": rolled_back,
            })
            .to_string(),
        },
    )
    .await?;

    Ok(BenchmarkOutcome::Completed {
        run_id: run.id,
        action_row_id,
        verdict,
        rolled_back,
        rollback_action_row_id,
    })
}

fn direction_str(direction: MetricDirection) -> &'static str {
    match direction {
        MetricDirection::LowerIsBetter => "LOWER_IS_BETTER",
        MetricDirection::HigherIsBetter => "HIGHER_IS_BETTER",
    }
}
