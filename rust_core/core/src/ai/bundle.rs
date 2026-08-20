//! Turns U5's deterministic verdicts into the numbered, referenceable shape
//! the AI provider is allowed to see (SRS FR-AI-002/004; TRS §23). Every
//! function here is pure — no I/O, no provider dependency, matching
//! `core::analysis`'s own boundary in the other direction: `core::ai`
//! consumes `analysis::Evidence`/`HostVerdict`/`DbHealthVerdict`, never the
//! reverse.

use contracts::telemetry::{MetricValue, ProcessCategory, ProcessInfo};

use crate::analysis::{DbEvidenceBundle, DbHealthVerdict, Evidence, HostBottleneck, HostVerdict};
use crate::dbms::SessionState;

#[derive(Debug, Clone)]
pub struct EvidenceItem {
    pub id: u32,
    pub metric: String,
    pub observed: f64,
    pub threshold: f64,
    pub unit: Option<String>,
}

/// A referenceable entity (a PID, a table, ...) — untrusted text (`label`)
/// that must be placed in the prompt's single delimited data block (TRS
/// §23), never treated as instruction-shaped.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: u32,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct EvidenceBundle {
    pub subject: String,
    pub verdict_label: String,
    pub evidence: Vec<EvidenceItem>,
    pub candidates: Vec<Candidate>,
}

fn evidence_items(evidence: &[Evidence]) -> Vec<EvidenceItem> {
    evidence
        .iter()
        .enumerate()
        .map(|(i, e)| EvidenceItem {
            id: i as u32,
            metric: e.metric.clone(),
            observed: e.observed,
            threshold: e.threshold,
            unit: e.unit.clone(),
        })
        .collect()
}

fn host_bottleneck_label(b: HostBottleneck) -> &'static str {
    match b {
        HostBottleneck::None => "NONE",
        HostBottleneck::Cpu => "CPU",
        HostBottleneck::Memory => "MEMORY",
        HostBottleneck::StorageIo => "STORAGE_IO",
        HostBottleneck::Network => "NETWORK",
        HostBottleneck::Thermal => "THERMAL",
        HostBottleneck::Power => "POWER",
        HostBottleneck::Background => "BACKGROUND",
        HostBottleneck::Multiple => "MULTIPLE",
        HostBottleneck::Unknown => "UNKNOWN",
    }
}

fn cpu_percent(p: &ProcessInfo) -> Option<f64> {
    match &p.cpu_percent {
        MetricValue::Supported { value } => Some(*value),
        _ => None,
    }
}

/// Candidates for a BACKGROUND (or any) host verdict: the top
/// BackgroundService-categorized processes by CPU%, if the caller supplied
/// live process data — the same optional input `analysis::classify_host`
/// takes, and absent for the same reason there (flagged, not guessed).
fn host_candidates(processes: Option<&[ProcessInfo]>) -> Vec<Candidate> {
    let Some(processes) = processes else {
        return Vec::new();
    };
    let mut background: Vec<&ProcessInfo> = processes
        .iter()
        .filter(|p| p.category == ProcessCategory::BackgroundService)
        .collect();
    background.sort_by(|a, b| {
        cpu_percent(b)
            .unwrap_or(0.0)
            .total_cmp(&cpu_percent(a).unwrap_or(0.0))
    });
    background
        .into_iter()
        .take(10)
        .enumerate()
        .map(|(i, p)| Candidate {
            id: i as u32,
            kind: "process".to_string(),
            label: format!("pid {} ({})", p.pid, p.comm),
        })
        .collect()
}

pub fn build_host_bundle(
    verdict: &HostVerdict,
    subject: &str,
    processes: Option<&[ProcessInfo]>,
) -> EvidenceBundle {
    EvidenceBundle {
        subject: subject.to_string(),
        verdict_label: host_bottleneck_label(verdict.bottleneck).to_string(),
        evidence: evidence_items(&verdict.evidence),
        candidates: host_candidates(processes),
    }
}

fn db_category_label(verdict: &DbHealthVerdict) -> String {
    let worst = verdict
        .checks
        .iter()
        .filter(|c| c.status != crate::analysis::DbHealthStatus::Unavailable)
        .max_by_key(|c| match c.status {
            crate::analysis::DbHealthStatus::Ok => 0,
            crate::analysis::DbHealthStatus::Warning => 1,
            crate::analysis::DbHealthStatus::Critical => 2,
            crate::analysis::DbHealthStatus::Unavailable => unreachable!(),
        });
    match worst {
        Some(check) => format!("{:?}:{:?}", check.category, check.status),
        None => "UNKNOWN".to_string(),
    }
}

/// Candidates for a DB health verdict: long-running/idle-in-transaction
/// session PIDs and the table names behind the bloat-proxy check — real
/// entities the source data already names, not invented.
fn db_candidates(source: &DbEvidenceBundle) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for txn in &source.long_transactions {
        candidates.push(Candidate {
            id: candidates.len() as u32,
            kind: "session".to_string(),
            label: format!("pid {} (long transaction)", txn.pid),
        });
    }
    for session in source
        .sessions
        .iter()
        .filter(|s| s.state == SessionState::IdleInTransaction)
    {
        candidates.push(Candidate {
            id: candidates.len() as u32,
            kind: "session".to_string(),
            label: format!("pid {} (idle in transaction)", session.pid),
        });
    }
    for table in &source.table_stats {
        candidates.push(Candidate {
            id: candidates.len() as u32,
            kind: "table".to_string(),
            label: format!("{}.{}", table.schema, table.table),
        });
    }
    candidates
}

pub fn build_db_bundle(
    verdict: &DbHealthVerdict,
    source: &DbEvidenceBundle,
    subject: &str,
) -> EvidenceBundle {
    let evidence: Vec<Evidence> = verdict
        .checks
        .iter()
        .flat_map(|c| c.evidence.clone())
        .collect();
    EvidenceBundle {
        subject: subject.to_string(),
        verdict_label: db_category_label(verdict),
        evidence: evidence_items(&evidence),
        candidates: db_candidates(source),
    }
}
