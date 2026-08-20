//! Unit U19: host performance analysis, wired for the first time since
//! U5 shipped `classify_host` (ADR 0017 forward-referenced exactly this
//! gap). DB-side `classify_db`/`correlate()` stay out of scope — `service`
//! has no live `DbmsAdapter`/DB-connection concept anywhere yet (see
//! docs/adr/0024).

use ai_ops_core::analysis::thresholds::Tier as CoreTier;
use ai_ops_core::analysis::{self, HostBottleneck as CoreHostBottleneck, HostDomain, HostVerdict};
use ai_ops_core::repository;
use axum::extract::{Extension, State};
use chrono::{Duration, Utc};
use contracts::analysis::{
    DomainSignal, Evidence, HostBottleneck, HostVerdict as WireHostVerdict, Tier,
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
        evidence: verdict
            .evidence
            .into_iter()
            .map(|e| Evidence {
                metric: e.metric,
                observed: e.observed,
                threshold: e.threshold,
                unit: e.unit,
                window_start: e.window_start,
                window_end: e.window_end,
            })
            .collect(),
        score: verdict.score,
        score_version: verdict.score_version.to_string(),
    }
}

pub async fn host(
    State(state): State<AppState>,
    Extension(RequestId(request_id)): Extension<RequestId>,
) -> ApiResponse<WireHostVerdict> {
    let result = async {
        let repo = state.repository.clone().ok_or_else(|| {
            ApiError::Unavailable(
                "host analysis not available: repository layer did not start".to_string(),
            )
        })?;
        let now = Utc::now();
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
            let verdict = analysis::classify_host(&history, Some(&processes), now);
            Ok::<_, ApiError>(to_wire(verdict))
        })
        .await
        .map_err(|_| ApiError::Internal)?
    }
    .await;

    ApiResponse::new(request_id, result)
}
