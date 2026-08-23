//! Unit U19: host performance analysis, wired for the first time since
//! U5 shipped `classify_host` (ADR 0017 forward-referenced exactly this
//! gap). Unit U20 adds the DB side and full cross-layer correlation —
//! see docs/adr/0025 for the DB-connection wiring judgment calls
//! (plain-env-var password, all-or-nothing poll failure, "no DB
//! configured is not an error").

use ai_ops_core::ai::{
    build_correlation_bundle, build_db_bundle, build_host_bundle, try_explain,
    AiExplanation as CoreAiExplanation, MetricClaim as CoreMetricClaim,
    Observation as CoreObservation, Recommendation as CoreRecommendation,
    RiskLevel as CoreAiRiskLevel,
};
use ai_ops_core::analysis::thresholds::Tier as CoreTier;
use ai_ops_core::analysis::{
    self, DbEvidenceBundle, DbHealthVerdict, HostBottleneck as CoreHostBottleneck, HostDomain,
    HostVerdict,
};
use ai_ops_core::correlation::{self, RootCause as CoreRootCause};
use ai_ops_core::repository;
use axum::extract::{Extension, State};
use chrono::{DateTime, Duration, Utc};
use contracts::ai::{
    AiExplanation as WireAiExplanation, MetricClaim, Observation, Recommendation,
    RiskLevel as WireAiRiskLevel,
};
use contracts::analysis::{
    DomainSignal, Evidence, HostBottleneck, HostVerdict as WireHostVerdict, Tier,
};
use contracts::correlation::{
    CorrelationResult as WireCorrelationResult, Hypothesis, RootCause, RuledOut,
};
use contracts::{ApiError, GatedValue};

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

/// A real, documented judgment call, not an arbitrary number: the
/// sampler's normal interval is 1s
/// (`service::telemetry::sampler::NORMAL_INTERVAL`), so 5 minutes
/// comfortably clears `analysis::thresholds::MIN_SAMPLES_FOR_
/// CLASSIFICATION` (3) even under the 5s back-off interval. No query
/// parameter to override it this unit.
const LOOKBACK_MINUTES: i64 = 5;

fn to_wire_bottleneck(b: CoreHostBottleneck) -> HostBottleneck {
    match b {
        CoreHostBottleneck::None => HostBottleneck::None,
        CoreHostBottleneck::Cpu => HostBottleneck::Cpu,
        CoreHostBottleneck::Memory => HostBottleneck::Memory,
        CoreHostBottleneck::StorageIo => HostBottleneck::StorageIo,
        CoreHostBottleneck::Network => HostBottleneck::Network,
        CoreHostBottleneck::Thermal => HostBottleneck::Thermal,
        CoreHostBottleneck::Power => HostBottleneck::Power,
        CoreHostBottleneck::Background => HostBottleneck::Background,
        CoreHostBottleneck::Multiple => HostBottleneck::Multiple,
        CoreHostBottleneck::Unknown => HostBottleneck::Unknown,
    }
}

fn to_wire_domain(d: HostDomain) -> contracts::analysis::HostDomain {
    match d {
        HostDomain::Cpu => contracts::analysis::HostDomain::Cpu,
        HostDomain::Memory => contracts::analysis::HostDomain::Memory,
        HostDomain::StorageIo => contracts::analysis::HostDomain::StorageIo,
        HostDomain::Network => contracts::analysis::HostDomain::Network,
    }
}

fn to_wire_tier(t: CoreTier) -> Tier {
    match t {
        CoreTier::Normal => Tier::Normal,
        CoreTier::Elevated => Tier::Elevated,
        CoreTier::High => Tier::High,
        CoreTier::Critical => Tier::Critical,
    }
}

fn to_wire_evidence(e: ai_ops_core::analysis::Evidence) -> Evidence {
    Evidence {
        metric: e.metric,
        observed: e.observed,
        threshold: e.threshold,
        unit: e.unit,
        window_start: e.window_start,
        window_end: e.window_end,
    }
}

fn to_wire(verdict: HostVerdict) -> WireHostVerdict {
    WireHostVerdict {
        bottleneck: to_wire_bottleneck(verdict.bottleneck),
        domain_signals: verdict
            .domain_signals
            .into_iter()
            .map(|s| DomainSignal {
                domain: to_wire_domain(s.domain),
                tier: to_wire_tier(s.tier),
                sample_count: s.sample_count as u64,
                crossed_count: s.crossed_count as u64,
            })
            .collect(),
        evidence: verdict.evidence.into_iter().map(to_wire_evidence).collect(),
        score: verdict.score,
        score_version: verdict.score_version.to_string(),
    }
}

fn to_wire_root_cause(c: CoreRootCause) -> RootCause {
    match c {
        CoreRootCause::DbLocks => RootCause::DbLocks,
        CoreRootCause::DbConfiguration => RootCause::DbConfiguration,
        CoreRootCause::ConnectionExhaustion => RootCause::ConnectionExhaustion,
        CoreRootCause::SlowSql => RootCause::SlowSql,
        CoreRootCause::HostCpu => RootCause::HostCpu,
        CoreRootCause::HostMemory => RootCause::HostMemory,
        CoreRootCause::StorageLatency => RootCause::StorageLatency,
        CoreRootCause::Network => RootCause::Network,
        CoreRootCause::ClientSideApplication => RootCause::ClientSideApplication,
    }
}

fn to_wire_correlation(result: correlation::CorrelationResult) -> WireCorrelationResult {
    WireCorrelationResult {
        window_start: result.window_start,
        window_end: result.window_end,
        ranked: result
            .ranked
            .into_iter()
            .map(|h| Hypothesis {
                cause: to_wire_root_cause(h.cause),
                confidence: h.confidence,
                evidence: h.evidence.into_iter().map(to_wire_evidence).collect(),
            })
            .collect(),
        ruled_out: result
            .ruled_out
            .into_iter()
            .map(|r| RuledOut {
                cause: to_wire_root_cause(r.cause),
                reason: r.reason,
            })
            .collect(),
    }
}

/// Shared by both `host` and `correlation` — same 5-minute-window,
/// `spawn_blocking` query logic either handler needs, unchanged from
/// U19's own single-endpoint version.
async fn compute_host_verdict(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<HostVerdict, ApiError> {
    let repo = state.repository.clone().ok_or_else(|| {
        ApiError::Unavailable(
            "host analysis not available: repository layer did not start".to_string(),
        )
    })?;
    let since = now - Duration::minutes(LOOKBACK_MINUTES);
    let processes = state
        .host_telemetry
        .read()
        .await
        .processes
        .processes
        .clone();

    tokio::task::spawn_blocking(move || {
        let conn = repository::reader::checkout(&repo.read_pool)
            .map_err(|err| ApiError::Unavailable(err.to_string()))?;
        let history = repository::query_recent_snapshots(&conn, since).map_err(|err| {
            tracing::error!(error = %err, "query_recent_snapshots failed");
            ApiError::Internal
        })?;
        Ok::<_, ApiError>(analysis::classify_host(&history, Some(&processes), now))
    })
    .await
    .map_err(|_| ApiError::Internal)?
}

/// Unit U46: the real poll logic, split out from `compute_db_verdict`
/// so `explain_db` can reuse it without duplicating the 10-way
/// `try_join!` — `explain_db` needs the real `DbEvidenceBundle` itself
/// (for `core::ai::build_db_bundle`'s own candidates), not just the
/// classified verdict `compute_db_verdict` alone returns. All-or-
/// nothing on the poll: if any of the ~10 calls fails, the whole
/// bundle degrades rather than mixing real and fabricated fields.
async fn compute_db_verdict_and_bundle(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<(DbHealthVerdict, DbEvidenceBundle), String> {
    let adapter = state
        .dbms_adapter
        .clone()
        .ok_or_else(|| "no database connection configured".to_string())?;

    let (
        databases,
        sessions,
        query_stats,
        locks,
        table_stats,
        replication,
        gucs,
        temp_file_activity,
        deadlocks,
        long_transactions,
    ) = tokio::try_join!(
        adapter.list_databases(),
        adapter.list_sessions(),
        adapter.query_stats(),
        adapter.lock_graph(),
        adapter.table_stats(),
        adapter.replication_status(),
        adapter.relevant_gucs(),
        adapter.temp_file_activity(),
        adapter.deadlock_history(),
        adapter.long_transactions(),
    )
    .map_err(|err| {
        tracing::warn!(error = %err, "DB poll failed; correlation's DB side degrades to unavailable");
        format!("DB poll failed: {err}")
    })?;

    let bundle = DbEvidenceBundle {
        databases,
        sessions,
        query_stats,
        locks,
        table_stats,
        replication,
        gucs,
        temp_file_activity,
        deadlocks,
        long_transactions,
    };
    let verdict = analysis::classify_db(&bundle, now);
    Ok((verdict, bundle))
}

/// Never fails — "no DB evidence" (unconfigured, or a poll that failed)
/// is a real, honest verdict (`unavailable_verdict`), not an error this
/// endpoint refuses to answer.
async fn compute_db_verdict(state: &AppState, now: DateTime<Utc>) -> DbHealthVerdict {
    match compute_db_verdict_and_bundle(state, now).await {
        Ok((verdict, _)) => verdict,
        Err(reason) => analysis::unavailable_verdict(&reason, now),
    }
}

pub async fn host(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<WireHostVerdict> {
    let now = Utc::now();
    let result = compute_host_verdict(&state, now).await.map(to_wire);
    ApiResponse::new(request_id, result)
}

fn to_wire_ai_risk(risk: CoreAiRiskLevel) -> WireAiRiskLevel {
    match risk {
        CoreAiRiskLevel::Low => WireAiRiskLevel::Low,
        CoreAiRiskLevel::Medium => WireAiRiskLevel::Medium,
        CoreAiRiskLevel::High => WireAiRiskLevel::High,
        CoreAiRiskLevel::Critical => WireAiRiskLevel::Critical,
    }
}

fn to_wire_metric_claim(claim: CoreMetricClaim) -> MetricClaim {
    MetricClaim {
        value: claim.value,
        evidence_ref: claim.evidence_ref,
    }
}

fn to_wire_observation(observation: CoreObservation) -> Observation {
    Observation {
        text: observation.text,
        metrics: observation
            .metrics
            .into_iter()
            .map(to_wire_metric_claim)
            .collect(),
    }
}

fn to_wire_recommendation(recommendation: CoreRecommendation) -> Recommendation {
    Recommendation {
        text: recommendation.text,
        metrics: recommendation
            .metrics
            .into_iter()
            .map(to_wire_metric_claim)
            .collect(),
        candidate_ref: recommendation.candidate_ref,
    }
}

fn to_wire_ai_explanation(explanation: CoreAiExplanation) -> WireAiExplanation {
    WireAiExplanation {
        summary: explanation.summary,
        observations: explanation
            .observations
            .into_iter()
            .map(to_wire_observation)
            .collect(),
        recommendations: explanation
            .recommendations
            .into_iter()
            .map(to_wire_recommendation)
            .collect(),
        risk: to_wire_ai_risk(explanation.risk),
        confidence: explanation.confidence,
    }
}

/// Unit U44's first direct wire surface for `core::ai` — fully built
/// since unit U6, but until now consumed by nothing in `service` at
/// all. Reuses `compute_host_verdict` unchanged; degrades to a real
/// `503` when no provider is configured, or a real `200` +
/// `GatedValue::Unavailable` when a configured provider's own
/// `try_explain` degrades (absent/unreachable/timeout/garbage/an
/// unresolved citation — `try_explain`'s own contract deliberately
/// discards which one, SRS FR-AI-001, so this can't be more specific
/// without being dishonest). See docs/adr/0049.
pub async fn explain_host(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<GatedValue<WireAiExplanation>> {
    let now = Utc::now();
    let result = async {
        let provider = state
            .ai_provider
            .clone()
            .ok_or_else(|| ApiError::Unavailable("no AI provider configured".to_string()))?;
        let verdict = compute_host_verdict(&state, now).await?;
        let processes = state
            .host_telemetry
            .read()
            .await
            .processes
            .processes
            .clone();
        let bundle = build_host_bundle(&verdict, "HOST", Some(&processes));
        Ok(match try_explain(provider.as_ref(), &bundle).await {
            Some(explanation) => GatedValue::Supported {
                value: to_wire_ai_explanation(explanation),
            },
            None => GatedValue::Unavailable {
                reason: "AI explanation unavailable".to_string(),
            },
        })
    }
    .await;

    ApiResponse::new(request_id, result)
}

/// Unit U46: `core::ai`'s second and final planned slice
/// (`build_db_bundle` — `build_host_bundle`'s own sibling). Unlike
/// `explain_host`, a real `DbEvidenceBundle` (not just a verdict) is
/// required — `compute_db_verdict_and_bundle` degrades to a real `503`
/// with its own real reason ("no database connection configured" or a
/// real poll failure) rather than fabricating an empty bundle to
/// explain, the same "no real evidence, no fabricated 200" precedent
/// every other DBMS-dependent endpoint in this codebase already uses.
/// See docs/adr/0051.
pub async fn explain_db(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<GatedValue<WireAiExplanation>> {
    let now = Utc::now();
    let result = async {
        let provider = state
            .ai_provider
            .clone()
            .ok_or_else(|| ApiError::Unavailable("no AI provider configured".to_string()))?;
        let (verdict, source) = compute_db_verdict_and_bundle(&state, now)
            .await
            .map_err(ApiError::Unavailable)?;
        let bundle = build_db_bundle(&verdict, &source, "DATABASE");
        Ok(match try_explain(provider.as_ref(), &bundle).await {
            Some(explanation) => GatedValue::Supported {
                value: to_wire_ai_explanation(explanation),
            },
            None => GatedValue::Unavailable {
                reason: "AI explanation unavailable".to_string(),
            },
        })
    }
    .await;

    ApiResponse::new(request_id, result)
}

pub async fn correlation(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<WireCorrelationResult> {
    let now = Utc::now();
    let result = async {
        let host_verdict = compute_host_verdict(&state, now).await?;
        let db_verdict = compute_db_verdict(&state, now).await;
        let since = now - Duration::minutes(LOOKBACK_MINUTES);
        let result = correlation::correlate(&host_verdict, &db_verdict, since, now);
        Ok::<_, ApiError>(to_wire_correlation(result))
    }
    .await;

    ApiResponse::new(request_id, result)
}

/// Unit U70: `explain_host`/`explain_db`'s third sibling —
/// `build_correlation_bundle`'s own real `correlate()` output, run
/// through the identical `try_explain` degrade contract (absent/
/// unreachable/timeout/garbage/an unresolved citation all collapse to
/// the same honest `Unavailable`, SRS FR-AI-001). Recomputes
/// `correlate()` fresh rather than reusing `correlation`'s handler
/// above — the same "no caching a stale verdict across two independent
/// requests" precedent every other explain endpoint already follows.
pub async fn explain_correlation(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<GatedValue<WireAiExplanation>> {
    let now = Utc::now();
    let result = async {
        let provider = state
            .ai_provider
            .clone()
            .ok_or_else(|| ApiError::Unavailable("no AI provider configured".to_string()))?;
        let host_verdict = compute_host_verdict(&state, now).await?;
        let db_verdict = compute_db_verdict(&state, now).await;
        let since = now - Duration::minutes(LOOKBACK_MINUTES);
        let result = correlation::correlate(&host_verdict, &db_verdict, since, now);
        let bundle = build_correlation_bundle(&result, "CORRELATION");
        Ok(match try_explain(provider.as_ref(), &bundle).await {
            Some(explanation) => GatedValue::Supported {
                value: to_wire_ai_explanation(explanation),
            },
            None => GatedValue::Unavailable {
                reason: "AI explanation unavailable".to_string(),
            },
        })
    }
    .await;

    ApiResponse::new(request_id, result)
}
