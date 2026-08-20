//! Host bottleneck classification (SRS FR-PERF-001/002, "the differentiator"
//! per SRS §11). `classify_host` is pure and synchronous: no I/O, no clock
//! reads beyond the explicit `now` parameter, no AI dependency of any kind
//! (FR-PERF-005) — fully testable with hand-built `TelemetrySnapshotRow`
//! fixtures.

use chrono::{DateTime, Utc};
use contracts::telemetry::{
    CpuPressure, MemoryPressure, NetworkInterfaceInfo, ProcessInfo, VolumeInfo,
};

use super::evidence::Evidence;
use super::thresholds::{
    self, Tier, BACKGROUND_MAJORITY_FRACTION, MIN_SAMPLES_FOR_CLASSIFICATION, SUSTAINED_FRACTION,
};
use crate::repository::TelemetrySnapshotRow;

/// SRS FR-PERF-001's exact ten values. THERMAL and POWER are real,
/// reachable variants (the SRS enum can't be trimmed) but no platform
/// exposes thermal or power data today (see docs/adr/0010) — the
/// classifier below never selects either, a fact `analysis_host_
/// classification_test.rs` asserts directly rather than leaving implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostBottleneck {
    None,
    Cpu,
    Memory,
    StorageIo,
    Network,
    Thermal,
    Power,
    Background,
    Multiple,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostDomain {
    Cpu,
    Memory,
    StorageIo,
    Network,
}

#[derive(Debug, Clone, Copy)]
pub struct DomainSignal {
    pub domain: HostDomain,
    pub tier: Tier,
    pub sample_count: usize,
    pub crossed_count: usize,
}

impl DomainSignal {
    /// Crossed when there's enough data (`MIN_SAMPLES_FOR_CLASSIFICATION`)
    /// and at least `SUSTAINED_FRACTION` of samples are High/Critical —
    /// a sustained-signal rule, not single-sample noise, matching the
    /// spirit of FR-SYS-003's existing sampler back-off.
    pub fn crossed(&self) -> bool {
        self.sample_count >= MIN_SAMPLES_FOR_CLASSIFICATION
            && (self.crossed_count as f64 / self.sample_count as f64) >= SUSTAINED_FRACTION
    }
}

#[derive(Debug, Clone)]
pub struct HostVerdict {
    pub bottleneck: HostBottleneck,
    pub domain_signals: Vec<DomainSignal>,
    pub evidence: Vec<Evidence>,
    pub score: u8,
    pub score_version: &'static str,
}

fn cpu_pressure_tier(p: CpuPressure) -> Tier {
    match p {
        CpuPressure::Normal => Tier::Normal,
        CpuPressure::Elevated => Tier::Elevated,
        CpuPressure::High => Tier::High,
        CpuPressure::Critical => Tier::Critical,
    }
}

fn memory_pressure_tier(p: MemoryPressure) -> Tier {
    match p {
        MemoryPressure::Normal => Tier::Normal,
        MemoryPressure::Elevated => Tier::Elevated,
        MemoryPressure::High => Tier::High,
        MemoryPressure::Critical => Tier::Critical,
    }
}

fn parse_cpu_pressure(s: &str) -> Option<CpuPressure> {
    match s {
        "NORMAL" => Some(CpuPressure::Normal),
        "ELEVATED" => Some(CpuPressure::Elevated),
        "HIGH" => Some(CpuPressure::High),
        "CRITICAL" => Some(CpuPressure::Critical),
        _ => None,
    }
}

fn parse_memory_pressure(s: &str) -> Option<MemoryPressure> {
    match s {
        "NORMAL" => Some(MemoryPressure::Normal),
        "ELEVATED" => Some(MemoryPressure::Elevated),
        "HIGH" => Some(MemoryPressure::High),
        "CRITICAL" => Some(MemoryPressure::Critical),
        _ => None,
    }
}

fn worst_storage_latency_ms(row: &TelemetrySnapshotRow) -> Option<f64> {
    let json = row.storage_volumes_json.as_ref()?;
    let volumes: Vec<VolumeInfo> = serde_json::from_str(json).ok()?;
    volumes
        .iter()
        .filter_map(|v| match &v.io_latency_ms {
            contracts::telemetry::MetricValue::Supported { value } => Some(*value),
            _ => None,
        })
        .fold(None, |acc, v| Some(acc.map_or(v, |a: f64| a.max(v))))
}

fn network_error_ratio(row: &TelemetrySnapshotRow) -> Option<f64> {
    let json = row.net_interfaces_json.as_ref()?;
    let interfaces: Vec<NetworkInterfaceInfo> = serde_json::from_str(json).ok()?;
    let supported = |mv: &contracts::telemetry::MetricValue<f64>| match mv {
        contracts::telemetry::MetricValue::Supported { value } => Some(*value),
        _ => None,
    };
    let mut errors = 0.0;
    let mut packets = 0.0;
    let mut any = false;
    for iface in &interfaces {
        if let Some(v) = supported(&iface.rx_errors_per_sec) {
            errors += v;
            any = true;
        }
        if let Some(v) = supported(&iface.tx_errors_per_sec) {
            errors += v;
            any = true;
        }
        if let Some(v) = supported(&iface.rx_drops_per_sec) {
            errors += v;
            any = true;
        }
        if let Some(v) = supported(&iface.tx_drops_per_sec) {
            errors += v;
            any = true;
        }
        if let Some(v) = supported(&iface.rx_packets_per_sec) {
            packets += v;
        }
        if let Some(v) = supported(&iface.tx_packets_per_sec) {
            packets += v;
        }
    }
    if !any {
        return None;
    }
    if packets <= 0.0 {
        // Errors/drops with no packet traffic to divide by is itself a
        // signal (not "no data") — report the raw error count as the
        // ratio numerator against a floor of 1 packet, avoiding a bogus
        // infinite ratio while still surfacing non-zero errors.
        return Some(errors);
    }
    Some(errors / packets)
}

fn build_domain_signal(domain: HostDomain, tiers: impl Iterator<Item = Tier>) -> DomainSignal {
    let mut sample_count = 0;
    let mut crossed_count = 0;
    let mut worst = Tier::Normal;
    for tier in tiers {
        sample_count += 1;
        if tier.is_crossed() {
            crossed_count += 1;
        }
        worst = worst.max(tier);
    }
    DomainSignal {
        domain,
        tier: worst,
        sample_count,
        crossed_count,
    }
}

/// Sum of a `BackgroundService` process attribute against the total, used
/// to decide whether a crossed CPU/MEMORY domain is better explained as
/// BACKGROUND. `None` when `processes` wasn't supplied — this refinement
/// is skipped entirely rather than guessed (see docs/adr/0010).
fn background_share(
    processes: &[ProcessInfo],
    metric: impl Fn(&ProcessInfo) -> Option<f64>,
) -> Option<f64> {
    let mut total = 0.0;
    let mut background = 0.0;
    let mut any = false;
    for p in processes {
        if let Some(v) = metric(p) {
            any = true;
            total += v;
            if p.category == contracts::telemetry::ProcessCategory::BackgroundService {
                background += v;
            }
        }
    }
    if !any || total <= 0.0 {
        return None;
    }
    Some(background / total)
}

fn cpu_percent(p: &ProcessInfo) -> Option<f64> {
    match &p.cpu_percent {
        contracts::telemetry::MetricValue::Supported { value } => Some(*value),
        _ => None,
    }
}

fn rss_bytes(p: &ProcessInfo) -> Option<f64> {
    match &p.rss_bytes {
        contracts::telemetry::MetricValue::Supported { value } => Some(*value as f64),
        _ => None,
    }
}

/// Classifies the host bottleneck (if any) over `history`, a chronological
/// window of already-persisted samples. `processes` is an optional live
/// process snapshot used only to distinguish BACKGROUND from CPU/MEMORY —
/// when absent, BACKGROUND is never produced (a flagged degradation, not a
/// guess). `now` bounds the evidence window and drives the "insufficient
/// data" case.
pub fn classify_host(
    history: &[TelemetrySnapshotRow],
    processes: Option<&[ProcessInfo]>,
    now: DateTime<Utc>,
) -> HostVerdict {
    if history.is_empty() {
        return HostVerdict {
            bottleneck: HostBottleneck::Unknown,
            domain_signals: Vec::new(),
            evidence: vec![Evidence::new(
                "sample_count",
                0.0,
                MIN_SAMPLES_FOR_CLASSIFICATION as f64,
                None,
                now,
                now,
            )],
            score: 0,
            score_version: super::score::HOST_SCORE_VERSION,
        };
    }

    let window_start = history.iter().map(|r| r.ts).min().unwrap_or(now);
    let window_end = history.iter().map(|r| r.ts).max().unwrap_or(now);

    let cpu_signal = build_domain_signal(
        HostDomain::Cpu,
        history
            .iter()
            .filter_map(|r| parse_cpu_pressure(&r.cpu_pressure))
            .map(cpu_pressure_tier),
    );
    let memory_signal = build_domain_signal(
        HostDomain::Memory,
        history
            .iter()
            .filter_map(|r| parse_memory_pressure(&r.mem_pressure))
            .map(memory_pressure_tier),
    );
    let storage_signal = build_domain_signal(
        HostDomain::StorageIo,
        history
            .iter()
            .filter_map(worst_storage_latency_ms)
            .map(thresholds::classify_storage_io_tier),
    );
    let network_signal = build_domain_signal(
        HostDomain::Network,
        history
            .iter()
            .filter_map(network_error_ratio)
            .map(thresholds::classify_network_error_tier),
    );

    let domain_signals = vec![cpu_signal, memory_signal, storage_signal, network_signal];

    let mut evidence = Vec::new();
    for signal in &domain_signals {
        if signal.sample_count == 0 {
            continue;
        }
        let (metric, threshold, unit) = match signal.domain {
            HostDomain::Cpu => ("cpu_pressure_sustained_fraction", SUSTAINED_FRACTION, None),
            HostDomain::Memory => (
                "memory_pressure_sustained_fraction",
                SUSTAINED_FRACTION,
                None,
            ),
            HostDomain::StorageIo => (
                "storage_io_latency_sustained_fraction",
                SUSTAINED_FRACTION,
                None,
            ),
            HostDomain::Network => (
                "network_error_ratio_sustained_fraction",
                SUSTAINED_FRACTION,
                None,
            ),
        };
        evidence.push(Evidence::new(
            metric,
            signal.crossed_count as f64 / signal.sample_count as f64,
            threshold,
            unit,
            window_start,
            window_end,
        ));
    }

    let crossed: Vec<&DomainSignal> = domain_signals.iter().filter(|s| s.crossed()).collect();

    let bottleneck = match crossed.as_slice() {
        [] => HostBottleneck::None,
        [only] => {
            let background_fraction = processes.and_then(|procs| match only.domain {
                HostDomain::Cpu => background_share(procs, cpu_percent),
                HostDomain::Memory => background_share(procs, rss_bytes),
                _ => None,
            });
            match background_fraction {
                Some(fraction) if fraction >= BACKGROUND_MAJORITY_FRACTION => {
                    evidence.push(Evidence::new(
                        "background_service_share",
                        fraction,
                        BACKGROUND_MAJORITY_FRACTION,
                        None,
                        window_end,
                        window_end,
                    ));
                    HostBottleneck::Background
                }
                _ => match only.domain {
                    HostDomain::Cpu => HostBottleneck::Cpu,
                    HostDomain::Memory => HostBottleneck::Memory,
                    HostDomain::StorageIo => HostBottleneck::StorageIo,
                    HostDomain::Network => HostBottleneck::Network,
                },
            }
        }
        _ => HostBottleneck::Multiple,
    };

    let score = super::score::score_host(&domain_signals);

    HostVerdict {
        bottleneck,
        domain_signals,
        evidence,
        score,
        score_version: super::score::HOST_SCORE_VERSION,
    }
}
