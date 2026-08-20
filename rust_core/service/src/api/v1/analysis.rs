//! Unit U19: host performance analysis, wired for the first time since
//! U5 shipped `classify_host` (ADR 0017 forward-referenced exactly this
//! gap). Unit U20 adds the DB side and full cross-layer correlation —
//! see docs/adr/0025 for the DB-connection wiring judgment calls
//! (plain-env-var password, all-or-nothing poll failure, "no DB
//! configured is not an error").

use ai_ops_core::analysis::thresholds::Tier as CoreTier;
use ai_ops_core::analysis::{
    self, DbEvidenceBundle, DbHealthVerdict, HostBottleneck as CoreHostBottleneck, HostDomain,
    HostVerdict,
};
use ai_ops_core::correlation::{self, RootCause as CoreRootCause};
use ai_ops_core::repository;
use axum::extract::{Extension, State};
use chrono::{DateTime, Duration, Utc};
use contracts::analysis::{
    DomainSignal, Evidence, HostBottleneck, HostVerdict as WireHostVerdict, Tier,
};
use contracts::correlation::{
    CorrelationResult as WireCorrelationResult, Hypothesis, RootCause, RuledOut,
};
use contracts::ApiError;

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

/// Never fails — "no DB evidence" (unconfigured, or a poll that failed)
/// is a real, honest verdict (`unavailable_verdict`), not an error this
/// endpoint refuses to answer. All-or-nothing on the poll: if any of the
/// ~10 calls `DbEvidenceBundle` needs fails, the whole bundle degrades
/// rather than mixing real and fabricated fields.
async fn compute_db_verdict(state: &AppState, now: DateTime<Utc>) -> DbHealthVerdict {
    let Some(adapter) = state.dbms_adapter.clone() else {
        return analysis::unavailable_verdict("no database connection configured", now);
    };

    let bundle = async {
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
        )?;
        Ok::<_, ai_ops_core::dbms::DbmsError>(DbEvidenceBundle {
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
        })
    }
    .await;

    match bundle {
        Ok(bundle) => analysis::classify_db(&bundle, now),
        Err(err) => {
            tracing::warn!(error = %err, "DB poll failed; correlation's DB side degrades to unavailable");
            analysis::unavailable_verdict(&format!("DB poll failed: {err}"), now)
        }
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
