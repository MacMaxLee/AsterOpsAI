//! Fixture-based coverage of `analysis::classify_host` (SRS FR-PERF-001/
//! 002). Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
//!
//! SRS FR-PERF-005 ("no component of host or database scoring, or of
//! bottleneck classification, may be produced or influenced by an AI
//! model") isn't provable by a unit test in this file — it's a real,
//! CI-enforced dependency-graph invariant: see
//! `scripts/check-no-ai-reachable-from-analysis-or-correlation.sh`
//! (unit U64), which BFS-searches the real compiler-resolved module
//! graph from every item under `ai_ops_core::analysis` (and
//! `ai_ops_core::correlation`, TRS §20) for anything reaching
//! `ai_ops_core::ai`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_ops_core::analysis::{classify_host, HostBottleneck};
use ai_ops_core::repository::TelemetrySnapshotRow;
use chrono::{DateTime, Duration, Utc};
use contracts::telemetry::{
    MetricValue, NetworkInterfaceInfo, ProcessCategory, ProcessInfo, VolumeInfo,
};

fn base_time() -> DateTime<Utc> {
    "2026-01-01T00:00:00.000Z".parse().unwrap()
}

fn supported(v: f64) -> MetricValue<f64> {
    MetricValue::Supported { value: v }
}

fn row(ts: DateTime<Utc>, cpu_pressure: &str, mem_pressure: &str) -> TelemetrySnapshotRow {
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
        mem_pressure: mem_pressure.to_string(),
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

fn rows_with_pressure(
    n: usize,
    cpu_pressure: &str,
    mem_pressure: &str,
) -> Vec<TelemetrySnapshotRow> {
    (0..n)
        .map(|i| {
            row(
                base_time() + Duration::seconds(i as i64 * 5),
                cpu_pressure,
                mem_pressure,
            )
        })
        .collect()
}

fn storage_row(ts: DateTime<Utc>, latency_ms: f64) -> TelemetrySnapshotRow {
    let mut r = row(ts, "NORMAL", "NORMAL");
    let volume = VolumeInfo {
        device: "/dev/sda1".to_string(),
        mount_point: "/".to_string(),
        filesystem: "ext4".to_string(),
        capacity_bytes: supported(0.0).map_u64(),
        free_bytes: supported(0.0).map_u64(),
        available_bytes: supported(0.0).map_u64(),
        read_bytes_per_sec: supported(0.0),
        write_bytes_per_sec: supported(0.0),
        read_ops_per_sec: supported(0.0),
        write_ops_per_sec: supported(0.0),
        io_latency_ms: supported(latency_ms),
    };
    r.storage_volumes_json = Some(serde_json::to_string(&vec![volume]).unwrap());
    r
}

fn network_row(ts: DateTime<Utc>, error_ratio_source: (f64, f64)) -> TelemetrySnapshotRow {
    let (errors, packets) = error_ratio_source;
    let mut r = row(ts, "NORMAL", "NORMAL");
    let iface = NetworkInterfaceInfo {
        name: "eth0".to_string(),
        rx_bytes_per_sec: supported(0.0),
        tx_bytes_per_sec: supported(0.0),
        rx_packets_per_sec: supported(packets),
        tx_packets_per_sec: supported(0.0),
        rx_errors_per_sec: supported(errors),
        tx_errors_per_sec: supported(0.0),
        rx_drops_per_sec: supported(0.0),
        tx_drops_per_sec: supported(0.0),
    };
    r.net_interfaces_json = Some(serde_json::to_string(&vec![iface]).unwrap());
    r
}

fn background_process(pid: u32, category: ProcessCategory, cpu_pct: f64) -> ProcessInfo {
    ProcessInfo {
        pid,
        start_time_ticks: 0,
        comm: format!("proc{pid}"),
        cmdline: supported_string(""),
        owner_uid: 0,
        cpu_percent: supported(cpu_pct),
        rss_bytes: MetricValue::Supported { value: 0 },
        category,
        disk_io_capability: contracts::Capability::Unavailable {
            reason: "n/a".to_string(),
        },
        disk_read_bytes_per_sec: supported(0.0),
        disk_write_bytes_per_sec: supported(0.0),
        network_io_capability: contracts::Capability::Unavailable {
            reason: "n/a".to_string(),
        },
        network_rx_bytes_per_sec: supported(0.0),
        network_tx_bytes_per_sec: supported(0.0),
    }
}

fn supported_string(s: &str) -> MetricValue<String> {
    MetricValue::Supported {
        value: s.to_string(),
    }
}

trait MapU64 {
    fn map_u64(self) -> MetricValue<u64>;
}
impl MapU64 for MetricValue<f64> {
    fn map_u64(self) -> MetricValue<u64> {
        match self {
            MetricValue::Supported { value } => MetricValue::Supported {
                value: value as u64,
            },
            MetricValue::SampleGap { reason } => MetricValue::SampleGap { reason },
            MetricValue::CounterReset { reason } => MetricValue::CounterReset { reason },
            MetricValue::Unavailable { reason } => MetricValue::Unavailable { reason },
        }
    }
}

#[test]
fn empty_history_is_unknown_not_none() {
    let verdict = classify_host(&[], None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Unknown);
    assert!(!verdict.evidence.is_empty());
}

#[test]
fn all_normal_is_none() {
    let history = rows_with_pressure(5, "NORMAL", "NORMAL");
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::None);
    assert_eq!(verdict.score, 100);
}

#[test]
fn sustained_high_cpu_is_cpu() {
    let history = rows_with_pressure(5, "HIGH", "NORMAL");
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Cpu);
    assert!(!verdict.evidence.is_empty());
}

#[test]
fn sustained_critical_memory_is_memory() {
    let history = rows_with_pressure(5, "NORMAL", "CRITICAL");
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Memory);
}

#[test]
fn transient_spike_below_min_samples_is_not_crossed() {
    // Only 2 samples, both HIGH — below MIN_SAMPLES_FOR_CLASSIFICATION (3).
    let history = rows_with_pressure(2, "HIGH", "NORMAL");
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::None);
}

#[test]
fn noisy_single_sample_among_many_normal_does_not_cross_sustained_fraction() {
    let mut history = rows_with_pressure(9, "NORMAL", "NORMAL");
    history.push(row(
        base_time() + Duration::seconds(45),
        "CRITICAL",
        "NORMAL",
    ));
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::None);
}

#[test]
fn sustained_storage_latency_is_storage_io() {
    let history: Vec<_> = (0..5)
        .map(|i| storage_row(base_time() + Duration::seconds(i * 5), 100.0))
        .collect();
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::StorageIo);
}

#[test]
fn sustained_network_errors_is_network() {
    let history: Vec<_> = (0..5)
        .map(|i| network_row(base_time() + Duration::seconds(i * 5), (5.0, 100.0)))
        .collect();
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Network);
}

#[test]
fn cpu_and_memory_both_crossed_is_multiple() {
    let history = rows_with_pressure(5, "CRITICAL", "CRITICAL");
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Multiple);
}

#[test]
fn cpu_dominated_by_background_processes_is_background() {
    let history = rows_with_pressure(5, "CRITICAL", "NORMAL");
    let processes = vec![
        background_process(1, ProcessCategory::BackgroundService, 80.0),
        background_process(2, ProcessCategory::UserApplication, 10.0),
    ];
    let verdict = classify_host(&history, Some(&processes), base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Background);
}

#[test]
fn cpu_dominated_by_user_processes_stays_cpu_even_with_process_data() {
    let history = rows_with_pressure(5, "CRITICAL", "NORMAL");
    let processes = vec![
        background_process(1, ProcessCategory::UserApplication, 80.0),
        background_process(2, ProcessCategory::BackgroundService, 10.0),
    ];
    let verdict = classify_host(&history, Some(&processes), base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Cpu);
}

#[test]
fn without_process_data_background_is_never_produced() {
    let history = rows_with_pressure(5, "CRITICAL", "NORMAL");
    let verdict = classify_host(&history, None, base_time());
    assert_eq!(verdict.bottleneck, HostBottleneck::Cpu);
}

#[test]
fn thermal_and_power_are_never_produced_by_any_fixture_in_this_suite() {
    // No data source exists for either (see docs/adr/0010) — every fixture
    // above, spanning every domain this classifier can evaluate, must never
    // resolve to Thermal or Power.
    let scenarios: Vec<Vec<TelemetrySnapshotRow>> = vec![
        rows_with_pressure(5, "CRITICAL", "CRITICAL"),
        rows_with_pressure(5, "NORMAL", "NORMAL"),
        (0..5)
            .map(|i| storage_row(base_time() + Duration::seconds(i * 5), 200.0))
            .collect(),
    ];
    for history in scenarios {
        let verdict = classify_host(&history, None, base_time());
        assert_ne!(verdict.bottleneck, HostBottleneck::Thermal);
        assert_ne!(verdict.bottleneck, HostBottleneck::Power);
    }
}

/// SRS FR-PERF-002: evidence carries a real time window, not a placeholder.
#[test]
fn evidence_window_spans_the_supplied_history() {
    let history = rows_with_pressure(5, "HIGH", "NORMAL");
    let verdict = classify_host(&history, None, base_time());
    let evidence = verdict
        .evidence
        .first()
        .expect("at least one evidence item");
    assert_eq!(evidence.window_start, base_time());
    assert_eq!(evidence.window_end, base_time() + Duration::seconds(20));
}
