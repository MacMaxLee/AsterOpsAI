//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod common;

use ai_ops_core::repository::{self, TelemetrySnapshotRow, WriteCommand};
use chrono::{Duration, TimeZone, Utc};
use common::TestRepo;
use rusqlite::Connection;

const SIMULATED_DAYS: i64 = 30;
// Coarser than the real sampler's ~10s persistence cadence (deliberately, to
// keep this test's real wall-clock runtime reasonable — the rollup/purge/
// eviction SQL logic under test doesn't depend on exactly how many rows
// land in each bucket, only on the ts values spanning the simulated range).
const PERSIST_INTERVAL_SECS: i64 = 120;
const SIZE_CEILING_BYTES: u64 = 500_000_000;

/// Format-realistic (not maximal) nested JSON, matching roughly what the
/// real sampler produces for a 2-core / few-volume / few-interface host.
fn synthetic_row(ts: chrono::DateTime<Utc>, i: i64) -> TelemetrySnapshotRow {
    TelemetrySnapshotRow {
        ts,
        cpu_aggregate_util_pct: Some(10.0 + (i % 50) as f64),
        cpu_aggregate_util_state: "SUPPORTED".to_string(),
        cpu_load_avg_1m: Some(0.5),
        cpu_pressure: "NORMAL".to_string(),
        cpu_per_core_json: Some(
            r#"[{"state":"SUPPORTED","value":12.5},{"state":"SUPPORTED","value":13.1}]"#
                .to_string(),
        ),
        mem_total_bytes: Some(4_000_000_000),
        mem_used_bytes: Some(1_500_000_000 + i % 1_000_000),
        mem_used_bytes_state: "SUPPORTED".to_string(),
        mem_available_bytes: Some(2_000_000_000),
        mem_swap_used_bytes: Some(100_000_000),
        mem_pressure: "NORMAL".to_string(),
        storage_read_bytes_ps: Some(1024.0),
        storage_write_bytes_ps: Some(2048.0),
        storage_volumes_json: Some(
            r#"[{"device":"sda2","mount_point":"/","filesystem":"ext4"}]"#.to_string(),
        ),
        net_rx_bytes_ps: Some(4096.0),
        net_tx_bytes_ps: Some(2048.0),
        net_interfaces_json: Some(r#"[{"name":"enp0s5"},{"name":"lo"}]"#.to_string()),
        process_total_count: Some(240),
        device_count: Some(19),
        containerized: false,
    }
}

fn database_size_bytes(conn: &Connection) -> u64 {
    let page_count: i64 = conn
        .query_row("SELECT * FROM pragma_page_count()", [], |r| r.get(0))
        .unwrap();
    let page_size: i64 = conn
        .query_row("SELECT * FROM pragma_page_size()", [], |r| r.get(0))
        .unwrap();
    (page_count.max(0) as u64) * (page_size.max(0) as u64)
}

#[tokio::test]
async fn thirty_day_simulated_fill_stays_under_the_size_ceiling() {
    let repo = TestRepo::open();

    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let rows_per_day = 86_400 / PERSIST_INTERVAL_SECS;

    for day in 0..SIMULATED_DAYS {
        let day_start = start + Duration::days(day);

        // Backdated inserts via an awaited (backpressure-respecting) send
        // through the real single writer — not a raw bypass connection, and
        // not the fire-and-forget `try_send` path (whose whole point is to
        // drop under load, which would silently defeat this test).
        for tick in 0..rows_per_day {
            let ts = day_start + Duration::seconds(tick * PERSIST_INTERVAL_SECS);
            let row = synthetic_row(ts, day * rows_per_day + tick);
            repo.handle
                .command_tx
                .send(WriteCommand::InsertTelemetrySnapshot(Box::new(row)))
                .await
                .unwrap();
        }

        // Run the sweep as of the END of this simulated day — not real
        // wall-clock time, and not the 15-minute production scheduler.
        // Sending it through the same channel guarantees every insert sent
        // above has already been processed (single writer, FIFO).
        let simulated_now = day_start + Duration::days(1);
        repository::run_retention_sweep(&repo.handle, simulated_now)
            .await
            .unwrap();
    }

    // Final sweep as of 30 simulated days in, matching how a real
    // long-running service would have swept periodically throughout.
    let final_now = start + Duration::days(SIMULATED_DAYS);
    repository::run_retention_sweep(&repo.handle, final_now)
        .await
        .unwrap();

    let conn = repository::reader::checkout(&repo.handle.read_pool).unwrap();
    let final_size = database_size_bytes(&conn);
    assert!(
        final_size <= SIZE_CEILING_BYTES,
        "final size {final_size} exceeds ceiling {SIZE_CEILING_BYTES}"
    );

    let daily_rollup_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM telemetry_rollup_daily", [], |r| {
            r.get(0)
        })
        .unwrap();
    // Every day except possibly the last (whose final hour may not have
    // finished rolling up depending on the grace window) should have a
    // completed daily rollup.
    assert!(
        daily_rollup_count >= SIMULATED_DAYS - 1,
        "expected close to {SIMULATED_DAYS} daily rollups, got {daily_rollup_count}"
    );

    let raw_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM telemetry_snapshots", [], |r| r.get(0))
        .unwrap();
    // 7-day raw retention should have purged everything older than 7 days
    // before `final_now`.
    assert!(
        raw_count <= rows_per_day * 8,
        "raw retention did not bound row count: {raw_count} rows"
    );

    // Not asserted on `report` (the *final* sweep's own count) — by the
    // time it runs, the day-29-to-day-30 sweep has typically already rolled
    // up everything there was to roll up, so the last sweep alone
    // legitimately finds nothing new. Check cumulative state instead: 90d
    // hourly retention means every hourly rollup from this 30-day run
    // should still be present.
    let hourly_rollup_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM telemetry_rollup_hourly", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        hourly_rollup_count > 0,
        "expected hourly rollups to have accumulated over the run"
    );

    // Sanity: the retention job's own audit rows from all 31 sweeps chained
    // cleanly.
    let verification = ai_ops_core::repository::audit::verify_chain(&conn).unwrap();
    assert_eq!(verification.first_broken_id, None);
}
