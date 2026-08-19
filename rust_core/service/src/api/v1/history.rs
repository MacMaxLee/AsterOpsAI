//! History endpoints (requirement 7): `last_hour`/`last_24h`/`last_7d`/
//! `last_30d`/`custom`, served from whichever rollup tier
//! `ai_ops_core::repository::history::resolve_range` selects. No
//! `/processes`/`/devices` history — that detail isn't persisted (see the
//! U2 plan's scope note), so there's nothing to serve.

use ai_ops_core::repository::{self, HistoryRange};
use axum::extract::{Extension, Query, State};
use chrono::{DateTime, Utc};
use contracts::telemetry::{
    CpuHistoryPoint, HistoryResponse, MemoryHistoryPoint, NetworkHistoryPoint, StorageHistoryPoint,
};
use contracts::ApiError;
use serde::Deserialize;

use crate::middleware::RequestId;
use crate::response::ApiResponse;
use crate::state::AppState;

/// `#[serde(rename_all = "snake_case")]` alone doesn't insert an underscore
/// before a digit (`Last24h` would deserialize as `last24h`, not the
/// `last_24h` this API's contract actually specifies), so each
/// digit-containing variant is renamed explicitly.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeParam {
    LastHour,
    #[serde(rename = "last_24h")]
    Last24h,
    #[serde(rename = "last_7d")]
    Last7d,
    #[serde(rename = "last_30d")]
    Last30d,
    Custom,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub range: RangeParam,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

fn to_history_range(query: &HistoryQuery) -> Result<HistoryRange, ApiError> {
    match query.range {
        RangeParam::LastHour => Ok(HistoryRange::LastHour),
        RangeParam::Last24h => Ok(HistoryRange::Last24h),
        RangeParam::Last7d => Ok(HistoryRange::Last7d),
        RangeParam::Last30d => Ok(HistoryRange::Last30d),
        RangeParam::Custom => {
            let from = query
                .from
                .ok_or_else(|| ApiError::BadRequest("range=custom requires `from`".to_string()))?;
            let to = query
                .to
                .ok_or_else(|| ApiError::BadRequest("range=custom requires `to`".to_string()))?;
            Ok(HistoryRange::Custom { from, to })
        }
    }
}

fn to_response<T>(resolved: repository::ResolvedRange, points: Vec<T>) -> HistoryResponse<T> {
    HistoryResponse {
        requested_from: resolved.from,
        requested_to: resolved.to,
        resolved_tier: resolved.tier,
        truncated_from: resolved.truncated_from,
        points,
    }
}

macro_rules! history_handler {
    ($name:ident, $point:ty, $query_fn:path) => {
        pub async fn $name(
            State(state): State<AppState>,
            Extension(RequestId(request_id)): Extension<RequestId>,
            Query(query): Query<HistoryQuery>,
        ) -> ApiResponse<HistoryResponse<$point>> {
            let result = async {
                // Request validation (400) takes precedence over service
                // state (503) — a malformed request is wrong regardless of
                // whether the repository happens to be up.
                let range = to_history_range(&query)?;
                let repo = state.repository.clone().ok_or_else(|| {
                    ApiError::Unavailable(
                        "history not available: repository layer did not start".to_string(),
                    )
                })?;
                let resolved = repository::history::resolve_range(range, Utc::now());

                tokio::task::spawn_blocking(move || {
                    let conn = repository::reader::checkout(&repo.read_pool)
                        .map_err(|err| ApiError::Unavailable(err.to_string()))?;
                    let points = $query_fn(&conn, &resolved).map_err(|err| {
                        tracing::error!(error = %err, "history query failed");
                        ApiError::Internal
                    })?;
                    Ok::<_, ApiError>(to_response(resolved, points))
                })
                .await
                .map_err(|_| ApiError::Internal)?
            }
            .await;

            ApiResponse::new(request_id, result)
        }
    };
}

history_handler!(cpu_history, CpuHistoryPoint, repository::query_cpu_history);
history_handler!(
    memory_history,
    MemoryHistoryPoint,
    repository::query_memory_history
);
history_handler!(
    storage_history,
    StorageHistoryPoint,
    repository::query_storage_history
);
history_handler!(
    network_history,
    NetworkHistoryPoint,
    repository::query_network_history
);
