use std::collections::HashMap;

use contracts::telemetry::{MetricValue, StorageSnapshot, VolumeInfo};
use platform::linux::ProcSource;

use super::context::SampleContext;
use super::error::TelemetryError;
use super::rate::rate_per_second;

/// Pseudo-filesystems that never carry meaningful capacity/IO telemetry —
/// excluded from `/storage`'s volume list.
const PSEUDO_FILESYSTEMS: &[&str] = &[
    "sysfs",
    "proc",
    "cgroup2",
    "devtmpfs",
    "tmpfs",
    "devpts",
    "securityfs",
    "pstore",
    "efivarfs",
    "bpf",
    "autofs",
    "debugfs",
    "tracefs",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "binfmt_misc",
    "fusectl",
    "configfs",
];

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

struct Mount {
    major_minor: String,
    mount_point: String,
    filesystem: String,
}

fn parse_mountinfo(raw: &str) -> Vec<Mount> {
    let mut mounts = Vec::new();
    for line in raw.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let left_fields: Vec<&str> = left.split_whitespace().collect();
        let right_fields: Vec<&str> = right.split_whitespace().collect();
        let (Some(major_minor), Some(mount_point)) = (left_fields.get(2), left_fields.get(4))
        else {
            continue;
        };
        let Some(filesystem) = right_fields.first() else {
            continue;
        };
        if PSEUDO_FILESYSTEMS.contains(filesystem) {
            continue;
        }
        mounts.push(Mount {
            major_minor: major_minor.to_string(),
            mount_point: mount_point.to_string(),
            filesystem: filesystem.to_string(),
        });
    }
    mounts
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiskCounters {
    reads_completed: u64,
    sectors_read: u64,
    ms_reading: u64,
    writes_completed: u64,
    sectors_written: u64,
    ms_writing: u64,
}

fn parse_diskstats(raw: &str) -> HashMap<String, DiskCounters> {
    let mut map = HashMap::new();
    for line in raw.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let major_minor = format!("{}:{}", fields[0], fields[1]);
        let parse = |i: usize| {
            fields
                .get(i)
                .and_then(|f| f.parse::<u64>().ok())
                .unwrap_or(0)
        };
        map.insert(
            major_minor,
            DiskCounters {
                reads_completed: parse(3),
                sectors_read: parse(5),
                ms_reading: parse(6),
                writes_completed: parse(7),
                sectors_written: parse(9),
                ms_writing: parse(10),
            },
        );
    }
    map
}

pub type PrevDiskState = HashMap<String, DiskCounters>;

/// SRS FR-STO-002: I/O latency where the platform exposes it. Unit U62's
/// `storage_counter_reset` test (below) now drives the `CounterReset`
/// branch with a real before/after fixture pair; the `Supported`
/// (real value computed), `Unavailable` (`delta_ops == 0`), and
/// `SampleGap` branches remain implemented but unverified — every other
/// existing test calls this via a `prev: None` first sample.
fn latency_ms(prev: &DiskCounters, curr: &DiskCounters, ctx: &SampleContext) -> MetricValue<f64> {
    if ctx.elapsed > ctx.configured_interval * 2 {
        return MetricValue::SampleGap {
            reason: "elapsed exceeds 2x sample interval".to_string(),
        };
    }
    let prev_ms = prev.ms_reading + prev.ms_writing;
    let curr_ms = curr.ms_reading + curr.ms_writing;
    let prev_ops = prev.reads_completed + prev.writes_completed;
    let curr_ops = curr.reads_completed + curr.writes_completed;
    if curr_ms < prev_ms || curr_ops < prev_ops {
        return MetricValue::CounterReset {
            reason: "disk I/O counter decreased".to_string(),
        };
    }
    let delta_ops = curr_ops - prev_ops;
    if delta_ops == 0 {
        return MetricValue::Unavailable {
            reason: "no I/O operations in this interval".to_string(),
        };
    }
    MetricValue::Supported {
        value: (curr_ms - prev_ms) as f64 / delta_ops as f64,
    }
}

const SECTOR_BYTES: u64 = 512;

pub fn parse_storage_snapshot(
    source: &dyn ProcSource,
    ctx: &SampleContext,
    prev: Option<&PrevDiskState>,
) -> Result<(StorageSnapshot, PrevDiskState), TelemetryError> {
    let mountinfo_raw = require(source, "proc/self/mountinfo")?;
    let mounts = parse_mountinfo(&mountinfo_raw);

    let diskstats_raw = require(source, "proc/diskstats")?;
    let counters = parse_diskstats(&diskstats_raw);

    let unavailable = || MetricValue::Unavailable {
        reason: "insufficient samples yet".to_string(),
    };

    let mut volumes = Vec::new();
    for mount in &mounts {
        let curr = counters
            .get(&mount.major_minor)
            .copied()
            .unwrap_or_default();
        let prev_counters = prev.and_then(|p| p.get(&mount.major_minor));

        let (
            read_bytes_per_sec,
            write_bytes_per_sec,
            read_ops_per_sec,
            write_ops_per_sec,
            io_latency_ms,
        ) = match prev_counters {
            Some(p) => (
                rate_per_second(
                    p.sectors_read * SECTOR_BYTES,
                    curr.sectors_read * SECTOR_BYTES,
                    ctx,
                ),
                rate_per_second(
                    p.sectors_written * SECTOR_BYTES,
                    curr.sectors_written * SECTOR_BYTES,
                    ctx,
                ),
                rate_per_second(p.reads_completed, curr.reads_completed, ctx),
                rate_per_second(p.writes_completed, curr.writes_completed, ctx),
                latency_ms(p, &curr, ctx),
            ),
            None => (
                unavailable(),
                unavailable(),
                unavailable(),
                unavailable(),
                unavailable(),
            ),
        };

        volumes.push(VolumeInfo {
            device: mount.major_minor.clone(),
            mount_point: mount.mount_point.clone(),
            filesystem: mount.filesystem.clone(),
            // Capacity/free/available aren't available from any proc/sys
            // text file; the service sampler fills these in via
            // `platform::linux::storage::volume_capacity` (statvfs), the
            // one deliberate exception to the ProcSource-only rule.
            capacity_bytes: MetricValue::Unavailable {
                reason: "resolved by caller via statvfs".to_string(),
            },
            free_bytes: MetricValue::Unavailable {
                reason: "resolved by caller via statvfs".to_string(),
            },
            available_bytes: MetricValue::Unavailable {
                reason: "resolved by caller via statvfs".to_string(),
            },
            read_bytes_per_sec,
            write_bytes_per_sec,
            read_ops_per_sec,
            write_ops_per_sec,
            io_latency_ms,
        });
    }

    let snapshot = StorageSnapshot {
        timestamp: ctx.now,
        volumes,
    };

    Ok((snapshot, counters))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::testing::FixtureProcSource;

    fn fixed_ctx() -> SampleContext {
        SampleContext {
            now: "2026-01-01T00:00:00Z".parse().unwrap(),
            elapsed: std::time::Duration::from_secs(1),
            configured_interval: std::time::Duration::from_secs(1),
        }
    }

    #[test]
    fn idle() {
        let source = FixtureProcSource::scenario("idle");
        let (snapshot, _prev) = parse_storage_snapshot(&source, &fixed_ctx(), None).unwrap();
        insta::assert_json_snapshot!(snapshot);
    }

    /// SRS FR-STO-001: per-volume I/O rates, and a disk counter reset
    /// must never present as a fabricated negative rate — mirrors
    /// `network.rs::nic_counter_reset`'s exact two-phase shape, the
    /// first test in this file to pass a real `prev` sample (every
    /// other storage test here only ever sees `prev: None`, so the
    /// `CounterReset` branch was genuinely untested until now).
    #[test]
    fn storage_counter_reset() {
        let before = FixtureProcSource::scenario_phase("storage-counter-reset", "before");
        let after = FixtureProcSource::scenario_phase("storage-counter-reset", "after");
        let (_, prev) = parse_storage_snapshot(&before, &fixed_ctx(), None).unwrap();
        let (snapshot, _) = parse_storage_snapshot(&after, &fixed_ctx(), Some(&prev)).unwrap();
        let root = snapshot
            .volumes
            .iter()
            .find(|v| v.mount_point == "/")
            .unwrap();
        assert!(matches!(
            root.read_bytes_per_sec,
            MetricValue::CounterReset { .. }
        ));
        assert!(matches!(
            root.write_bytes_per_sec,
            MetricValue::CounterReset { .. }
        ));
        assert!(matches!(
            root.io_latency_ms,
            MetricValue::CounterReset { .. }
        ));
        insta::assert_json_snapshot!(snapshot);
    }

    fn counters(reads: u64, writes: u64, ms_reading: u64, ms_writing: u64) -> DiskCounters {
        DiskCounters {
            reads_completed: reads,
            sectors_read: 0,
            ms_reading,
            writes_completed: writes,
            sectors_written: 0,
            ms_writing,
        }
    }

    /// SRS FR-STO-002, remaining half of the branch coverage the module
    /// doc comment names as still open after `storage_counter_reset`
    /// (above) closed the `CounterReset` branch: the real computed value
    /// (`Supported`) and the no-activity case (`Unavailable`).
    #[test]
    fn latency_ms_supported_computes_average_ms_per_op() {
        let prev = counters(10, 5, 100, 50);
        let curr = counters(15, 8, 150, 80);
        let result = latency_ms(&prev, &curr, &fixed_ctx());
        match result {
            MetricValue::Supported { value } => assert_eq!(value, 10.0),
            other => panic!("expected Supported, got {other:?}"),
        }
    }

    #[test]
    fn latency_ms_unavailable_when_no_operations_occurred() {
        let prev = counters(10, 5, 100, 50);
        let curr = counters(10, 5, 100, 50);
        assert!(matches!(
            latency_ms(&prev, &curr, &fixed_ctx()),
            MetricValue::Unavailable { .. }
        ));
    }
}
