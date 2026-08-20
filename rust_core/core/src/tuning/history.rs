//! FR-TUNE-003's "a locally recorded history of improved outcomes" — the
//! real join `repository::benchmark::query_improved_run_exists` runs
//! against `benchmark_runs`+`actions` (unit U9's schema had no
//! `action_type` column of its own to query directly).

use crate::repository::{has_improved_benchmark_history, RepositoryHandle};

use super::error::TuningError;

pub async fn has_improved_history(
    handle: &RepositoryHandle,
    action_type: &str,
) -> Result<bool, TuningError> {
    has_improved_benchmark_history(handle, action_type)
        .await
        .map_err(Into::into)
}
