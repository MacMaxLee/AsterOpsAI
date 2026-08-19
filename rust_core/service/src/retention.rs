//! Drives `core::repository`'s retention sweep (hourly/daily rollups, age
//! purge, size-ceiling eviction — unit U2) on a fixed interval. The sweep
//! logic itself has been fully implemented and tested since U2, but was
//! never actually invoked by the running service — this closes that gap.
//! Spawned only when the repository layer started; a repository-less
//! service has nothing to sweep.

use std::time::Duration;

use ai_ops_core::repository::{self, RepositoryHandle};
use chrono::Utc;

/// Matches the cadence the U2 plan documented for this pipeline: frequent
/// enough that raw data doesn't grow unbounded between sweeps, infrequent
/// enough that it isn't meaningful CPU/IO overhead.
const SWEEP_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub fn spawn(handle: RepositoryHandle) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        // tokio::time::interval's first tick fires immediately; skip it so
        // a freshly started service doesn't sweep a database that hasn't
        // had time to accumulate anything worth rolling up yet.
        interval.tick().await;
        loop {
            interval.tick().await;
            match repository::run_retention_sweep(&handle, Utc::now()).await {
                Ok(report) => {
                    tracing::info!(
                        hourly_rollups_computed = report.detail.hourly_rollups_computed,
                        daily_rollups_computed = report.detail.daily_rollups_computed,
                        rows_deleted_raw = report.detail.rows_deleted_raw,
                        rows_deleted_hourly = report.detail.rows_deleted_hourly,
                        rows_deleted_daily = report.detail.rows_deleted_daily,
                        ceiling_breached = report.detail.ceiling_breached,
                        "retention sweep completed"
                    );
                }
                Err(err) => {
                    tracing::error!(error = %err, "retention sweep failed");
                }
            }
        }
    });
}
