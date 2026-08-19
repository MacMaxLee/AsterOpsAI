//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "repository/common/mod.rs"]
mod common;

use std::time::{Duration as StdDuration, Instant};

use ai_ops_core::repository::{TelemetrySnapshotRow, WriteCommand};
use chrono::Utc;
use common::TestRepo;
use rusqlite::ffi::ErrorCode;

const WRITE_COUNT: usize = 5000;
const READER_TASKS: usize = 8;
const READS_PER_TASK: usize = 200;
/// What this test actually proves, beyond "the pragma exists": `busy_timeout`
/// already prevents `SQLITE_BUSY` from ever reaching a caller as long as
/// contention resolves within 5s — trivially true if misconfigured checks
/// were the only concern. This threshold instead asserts that under real,
/// sustained concurrent read+write load against the real WAL-mode file,
/// contention resolves *fast* — a regression that silently pushed every
/// read close to the 5s ceiling would still show "zero SQLITE_BUSY errors"
/// but would fail this bound.
const MAX_ACCEPTABLE_P99: StdDuration = StdDuration::from_millis(250);

fn synthetic_row(i: i64) -> TelemetrySnapshotRow {
    TelemetrySnapshotRow {
        ts: Utc::now(),
        cpu_aggregate_util_pct: Some((i % 100) as f64),
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
        storage_read_bytes_ps: Some(0.0),
        storage_write_bytes_ps: Some(0.0),
        storage_volumes_json: None,
        net_rx_bytes_ps: Some(0.0),
        net_tx_bytes_ps: Some(0.0),
        net_interfaces_json: None,
        process_total_count: Some(100),
        device_count: Some(5),
        containerized: false,
    }
}

#[tokio::test]
async fn concurrent_reads_and_writes_never_surface_sqlite_busy() {
    let repo = TestRepo::open();
    let read_pool = repo.handle.read_pool.clone();
    let command_tx = repo.handle.command_tx.clone();

    let writer_task = tokio::spawn(async move {
        for i in 0..WRITE_COUNT {
            let row = synthetic_row(i as i64);
            command_tx
                .send(WriteCommand::InsertTelemetrySnapshot(Box::new(row)))
                .await
                .unwrap();
        }
    });

    let mut reader_tasks = Vec::new();
    for _ in 0..READER_TASKS {
        let pool = read_pool.clone();
        reader_tasks.push(tokio::task::spawn_blocking(move || {
            let mut latencies = Vec::with_capacity(READS_PER_TASK);
            let mut busy_count = 0usize;
            for _ in 0..READS_PER_TASK {
                let start = Instant::now();
                let conn = pool.get().expect("pool checkout should not fail");
                let result: Result<i64, rusqlite::Error> =
                    conn.query_row("SELECT COUNT(*) FROM telemetry_snapshots", [], |r| r.get(0));
                let elapsed = start.elapsed();
                match result {
                    Ok(_) => latencies.push(elapsed),
                    Err(rusqlite::Error::SqliteFailure(e, _))
                        if e.code == ErrorCode::DatabaseBusy =>
                    {
                        busy_count += 1;
                    }
                    Err(e) => panic!("unexpected query error: {e}"),
                }
            }
            (latencies, busy_count)
        }));
    }

    writer_task.await.unwrap();

    let mut all_latencies = Vec::new();
    let mut total_busy = 0;
    for task in reader_tasks {
        let (latencies, busy) = task.await.unwrap();
        total_busy += busy;
        all_latencies.extend(latencies);
    }

    assert_eq!(
        total_busy, 0,
        "observed {total_busy} SQLITE_BUSY errors under concurrent load"
    );

    all_latencies.sort();
    assert!(!all_latencies.is_empty());
    let p99_index = ((all_latencies.len() as f64) * 0.99) as usize;
    let p99 = all_latencies[p99_index.min(all_latencies.len() - 1)];
    assert!(
        p99 < MAX_ACCEPTABLE_P99,
        "p99 read latency {p99:?} exceeds {MAX_ACCEPTABLE_P99:?}"
    );
}
