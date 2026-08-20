//! Coverage of `ai::build_host_bundle`/`build_db_bundle` (SRS FR-AI-002/004):
//! real `analysis::HostVerdict`/`DbHealthVerdict` fixtures (built the same
//! way analysis_host_classification_test.rs/analysis_db_health_test.rs do)
//! must turn into correctly-numbered, resolvable bundles. Integration tests
//! are already test-only code; the workspace's unwrap/expect deny targets
//! production code paths, not `tests/*.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::ai::{build_db_bundle, build_host_bundle};
use ai_ops_core::analysis::{classify_db, classify_host, DbEvidenceBundle};
use ai_ops_core::dbms::capability::Gated;
use ai_ops_core::dbms::{
    DatabaseInfo, DeadlockInfo, GucValue, LongTransaction, ReplicationStatus, SessionInfo,
    SessionState, TempFileActivity,
};
use ai_ops_core::repository::TelemetrySnapshotRow;
use chrono::{DateTime, Duration, Utc};
use contracts::telemetry::{MetricValue, ProcessCategory, ProcessInfo};

fn base_time() -> DateTime<Utc> {
    "2026-01-01T00:00:00.000Z".parse().unwrap()
}

fn row(ts: DateTime<Utc>, cpu_pressure: &str) -> TelemetrySnapshotRow {
    TelemetrySnapshotRow {
        ts,
        cpu_aggregate_util_pct: Some(10.0),
        cpu_aggregate_util_state: "SUPPORTED".to_string(),
        cpu_load_avg_1m: Some(1.0),
        cpu_pressure: cpu_pressure.to_string(),
        cpu_per_core_json: None,
        mem_total_bytes: Some(1_000_000),
        mem_used_bytes: Some(500_000),
        mem_used_bytes_state: "SUPPORTED".to_string(),
        mem_available_bytes: Some(500_000),
        mem_swap_used_bytes: Some(0),
        mem_pressure: "NORMAL".to_string(),
        storage_read_bytes_ps: Some(0.0),
        storage_write_bytes_ps: Some(0.0),
        storage_volumes_json: None,
        net_rx_bytes_ps: Some(0.0),
        net_tx_bytes_ps: Some(0.0),
        net_interfaces_json: None,
        process_total_count: Some(10),
        device_count: Some(1),
        containerized: false,
    }
}

fn background_process(pid: u32, category: ProcessCategory, cpu_pct: f64) -> ProcessInfo {
    ProcessInfo {
        pid,
        start_time_ticks: 0,
        comm: format!("proc{pid}"),
        cmdline: MetricValue::Supported {
            value: String::new(),
        },
        owner_uid: 0,
        cpu_percent: MetricValue::Supported { value: cpu_pct },
        rss_bytes: MetricValue::Supported { value: 0 },
        category,
        disk_io_capability: contracts::Capability::Unavailable {
            reason: "n/a".to_string(),
        },
        disk_read_bytes_per_sec: MetricValue::Supported { value: 0.0 },
        disk_write_bytes_per_sec: MetricValue::Supported { value: 0.0 },
        network_io_capability: contracts::Capability::Unavailable {
            reason: "n/a".to_string(),
        },
        network_rx_bytes_per_sec: MetricValue::Supported { value: 0.0 },
        network_tx_bytes_per_sec: MetricValue::Supported { value: 0.0 },
    }
}

#[test]
fn host_bundle_evidence_ids_are_dense_and_resolvable() {
    let history: Vec<_> = (0..5)
        .map(|i| row(base_time() + Duration::seconds(i * 5), "HIGH"))
        .collect();
    let verdict = classify_host(&history, None, base_time());
    assert!(!verdict.evidence.is_empty());

    let bundle = build_host_bundle(&verdict, "HOST", None);
    assert_eq!(bundle.subject, "HOST");
    assert_eq!(bundle.verdict_label, "CPU");
    assert_eq!(bundle.evidence.len(), verdict.evidence.len());
    for (i, item) in bundle.evidence.iter().enumerate() {
        assert_eq!(item.id, i as u32);
    }
    assert!(bundle.candidates.is_empty(), "no process data supplied");
}

#[test]
fn host_bundle_candidates_come_from_background_processes() {
    let history: Vec<_> = (0..5)
        .map(|i| row(base_time() + Duration::seconds(i * 5), "CRITICAL"))
        .collect();
    let verdict = classify_host(&history, None, base_time());
    let processes = vec![
        background_process(1, ProcessCategory::BackgroundService, 80.0),
        background_process(2, ProcessCategory::UserApplication, 10.0),
    ];
    let bundle = build_host_bundle(&verdict, "HOST", Some(&processes));
    assert_eq!(bundle.candidates.len(), 1);
    assert_eq!(bundle.candidates[0].kind, "process");
    assert!(bundle.candidates[0].label.contains("pid 1"));
}

fn db_bundle_source() -> DbEvidenceBundle {
    DbEvidenceBundle {
        databases: vec![DatabaseInfo {
            name: "app".to_string(),
            size_bytes: 0,
            xact_commit: 100,
            xact_rollback: 0,
        }],
        sessions: vec![SessionInfo {
            pid: 42,
            username: Some("app".to_string()),
            database: Some("app".to_string()),
            state: SessionState::IdleInTransaction,
            client_addr: None,
            xact_start: None,
            query_start: None,
            query: None,
        }],
        query_stats: Gated::Unavailable {
            reason: "not loaded".to_string(),
        },
        locks: Vec::new(),
        table_stats: Vec::new(),
        replication: ReplicationStatus {
            is_primary: true,
            in_recovery: false,
            standbys: Vec::new(),
        },
        gucs: vec![GucValue {
            name: "max_connections".to_string(),
            setting: "100".to_string(),
            unit: None,
            source: "configuration file".to_string(),
        }],
        temp_file_activity: TempFileActivity {
            temp_files: 0,
            temp_bytes: 0,
            stats_reset: Some(base_time()),
        },
        deadlocks: DeadlockInfo {
            deadlocks: 0,
            stats_reset: Some(base_time()),
        },
        long_transactions: vec![LongTransaction {
            pid: 7,
            username: Some("app".to_string()),
            duration_seconds: 600.0,
            state: SessionState::Active,
            query: None,
        }],
    }
}

#[test]
fn db_bundle_evidence_ids_are_dense_and_candidates_include_sessions() {
    let source = db_bundle_source();
    let verdict = classify_db(&source, base_time());
    let bundle = build_db_bundle(&verdict, &source, "primary-db");

    assert_eq!(bundle.subject, "primary-db");
    for (i, item) in bundle.evidence.iter().enumerate() {
        assert_eq!(item.id, i as u32);
    }
    assert!(bundle
        .candidates
        .iter()
        .any(|c| c.kind == "session" && c.label.contains("pid 7")));
    assert!(bundle
        .candidates
        .iter()
        .any(|c| c.kind == "session" && c.label.contains("pid 42")));
}
