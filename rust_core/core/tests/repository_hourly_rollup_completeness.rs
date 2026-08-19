//! Regression test for a real bug found by a full-codebase scan: the
//! hourly rollup's grace-period `WHERE` clause originally filtered on each
//! *row's* age (`ts < cutoff`) rather than on whether its *bucket* had
//! fully elapsed. At the real ~15-minute production sweep cadence, that let
//! the still-forming current hour's early rows get grouped and inserted as
//! a "complete" rollup on the very first sweep to touch that hour; because
//! the insert is `OR IGNORE` (for idempotency), every later sweep's fuller
//! data for the same `bucket_start` was then silently discarded forever.
//! `repository_retention_30day_fill.rs` didn't catch this because it only
//! sweeps once per simulated *day*, long after every hour in it has fully
//! elapsed either way — this test sweeps at a realistic sub-hour cadence
//! instead, which is exactly the condition that triggered the bug.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod common;

use ai_ops_core::repository::{self, TelemetrySnapshotRow, WriteCommand};
use chrono::{Duration, TimeZone, Utc};
use common::TestRepo;

const SWEEP_INTERVAL_MINUTES: i64 = 15;

fn synthetic_row(ts: chrono::DateTime<Utc>) -> TelemetrySnapshotRow {
    TelemetrySnapshotRow {
        ts,
        cpu_aggregate_util_pct: Some(20.0),
        cpu_aggregate_util_state: "SUPPORTED".to_string(),
        cpu_load_avg_1m: Some(0.5),
        cpu_pressure: "NORMAL".to_string(),
        cpu_per_core_json: None,
        mem_total_bytes: Some(4_000_000_000),
        mem_used_bytes: Some(1_500_000_000),
        mem_used_bytes_state: "SUPPORTED".to_string(),
        mem_available_bytes: Some(2_000_000_000),
        mem_swap_used_bytes: Some(0),
        mem_pressure: "NORMAL".to_string(),
        storage_read_bytes_ps: Some(1024.0),
        storage_write_bytes_ps: Some(2048.0),
        storage_volumes_json: None,
        net_rx_bytes_ps: Some(4096.0),
        net_tx_bytes_ps: Some(2048.0),
        net_interfaces_json: None,
        process_total_count: Some(120),
        device_count: Some(10),
        containerized: false,
    }
}

#[tokio::test]
async fn an_hour_swept_mid_flight_still_rolls_up_every_sample_once_complete() {
    let repo = TestRepo::open();

    let hour_start = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
    // One row per minute for the whole hour: 60 rows total.
    let all_rows: Vec<_> = (0..60).map(|m| hour_start + Duration::minutes(m)).collect();

    // Insert the whole hour's rows up front (order doesn't matter to the
    // rollup SQL — only `ts` values do), then sweep repeatedly at the real
    // ~15-minute cadence as simulated time advances through and past the
    // hour, exactly like the production service::retention loop does.
    for &ts in &all_rows {
        repo.handle
            .command_tx
            .send(WriteCommand::InsertTelemetrySnapshot(Box::new(
                synthetic_row(ts),
            )))
            .await
            .unwrap();
    }

    // Sweep mid-hour (10:15, 10:30, 10:45) — the still-forming 10:00 bucket
    // must NOT be rolled up on any of these.
    for offset_minutes in [15, 30, 45] {
        let simulated_now = hour_start + Duration::minutes(offset_minutes);
        repository::run_retention_sweep(&repo.handle, simulated_now)
            .await
            .unwrap();

        let conn = repository::reader::checkout(&repo.handle.read_pool).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM telemetry_rollup_hourly WHERE bucket_start = '2026-01-01T10:00:00.000Z'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "hour rolled up prematurely at +{offset_minutes}m into it"
        );
    }

    // Sweep once the hour plus grace period has fully elapsed.
    let after_hour = hour_start + Duration::hours(1) + Duration::minutes(SWEEP_INTERVAL_MINUTES);
    repository::run_retention_sweep(&repo.handle, after_hour)
        .await
        .unwrap();

    let conn = repository::reader::checkout(&repo.handle.read_pool).unwrap();
    let (total_count, supported_count): (i64, i64) = conn
        .query_row(
            "SELECT cpu_util_total_count, cpu_util_supported_count FROM telemetry_rollup_hourly \
             WHERE bucket_start = '2026-01-01T10:00:00.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    // The bug: this would read 45 (only the rows present at the first
    // sweep that happened to satisfy the old per-row filter) instead of
    // 60 (every row actually inserted for the hour), because `INSERT OR
    // IGNORE` silently discarded every subsequent sweep's fuller data.
    assert_eq!(
        total_count, 60,
        "hourly rollup is missing rows inserted before the bucket fully elapsed \
         — the grace-period filter let a partial rollup win permanently"
    );
    assert_eq!(supported_count, 60);
}
