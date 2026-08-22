use std::collections::HashMap;

use contracts::telemetry::{MetricValue, ProcessInfo, ProcessSnapshot};
use contracts::Capability;
use platform::linux::ProcSource;

use super::context::SampleContext;
use super::error::TelemetryError;
use super::process_classify::{classify_process, ProcessClassifyInput};
use super::rate::rate_per_second;

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PrevProcessCounters {
    cpu_ticks: u64,
    disk_read_bytes: u64,
    disk_write_bytes: u64,
}

pub type PrevProcessState = HashMap<u32, PrevProcessCounters>;

/// Hertz used to convert `/proc/[pid]/stat` jiffy fields to seconds. True on
/// every mainstream Linux target (x86_64, aarch64) for over a decade; the
/// exact value is technically `sysconf(_SC_CLK_TCK)`, but a live syscall
/// wrapper for a value that's been constant this long isn't worth the added
/// surface for this unit.
const USER_HZ: f64 = 100.0;

struct StatFields {
    comm: String,
    starttime: u64,
    utime: u64,
    stime: u64,
}

fn parse_stat_line(raw: &str) -> Option<StatFields> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    if close < open {
        return None;
    }
    let comm = raw[open + 1..close].to_string();
    let rest: Vec<&str> = raw[close + 1..].split_whitespace().collect();

    // Standard proc(5) numbering: pid=1, comm=2, state=3, ... utime=14,
    // stime=15, starttime=22. `rest[0]` is field 3 (state), so field N is
    // `rest[N - 3]`.
    let field = |overall: usize| rest.get(overall - 3).and_then(|s| s.parse::<u64>().ok());

    Some(StatFields {
        comm,
        utime: field(14)?,
        stime: field(15)?,
        starttime: field(22)?,
    })
}

/// Shared with `core::actions::host::target_verifier` (unit U8's real
/// `TargetVerifier`, which needs the exact same field-22 read to
/// re-verify a live process's identity before acting on it) — one
/// canonical `/proc/[pid]/stat` parse, not a second copy in that module.
pub(crate) fn read_start_time_ticks(source: &dyn ProcSource, pid: u32) -> Option<u64> {
    let raw = source.read(&format!("proc/{pid}/stat")).ok()?;
    parse_stat_line(&raw).map(|fields| fields.starttime)
}

fn parse_status(raw: &str) -> (Option<u32>, MetricValue<u64>) {
    let mut uid = None;
    let mut rss_bytes = MetricValue::Unavailable {
        reason: "VmRSS not present in /proc/[pid]/status".to_string(),
    };
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            uid = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        } else if let Some(rest) = line.strip_prefix("VmRSS:") {
            if let Some(kb) = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
            {
                rss_bytes = MetricValue::Supported { value: kb * 1024 };
            }
        }
    }
    (uid, rss_bytes)
}

fn parse_cmdline(raw: &str) -> MetricValue<String> {
    let joined = raw
        .split('\0')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        MetricValue::Unavailable {
            reason: "empty cmdline (likely a kernel thread)".to_string(),
        }
    } else {
        MetricValue::Supported { value: joined }
    }
}

fn parse_io_bytes(raw: &str) -> (u64, u64) {
    let mut read_bytes = 0u64;
    let mut write_bytes = 0u64;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("read_bytes:") {
            read_bytes = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("write_bytes:") {
            write_bytes = rest.trim().parse().unwrap_or(0);
        }
    }
    (read_bytes, write_bytes)
}

#[allow(clippy::too_many_lines)]
fn parse_one_process(
    source: &dyn ProcSource,
    pid: u32,
    ctx: &SampleContext,
    prev: Option<&PrevProcessCounters>,
) -> Option<(ProcessInfo, PrevProcessCounters)> {
    let stat_raw = source.read(&format!("proc/{pid}/stat")).ok()?;
    let stat = parse_stat_line(&stat_raw)?;

    let status_raw = source.read(&format!("proc/{pid}/status")).ok()?;
    let (uid, rss_bytes) = parse_status(&status_raw);
    let owner_uid = uid.unwrap_or(0);

    let cmdline_raw = source.read(&format!("proc/{pid}/cmdline")).ok();
    let cmdline_is_empty = cmdline_raw
        .as_deref()
        .map(|raw| raw.split('\0').all(|s| s.is_empty()))
        .unwrap_or(true);
    let cmdline = cmdline_raw
        .map(|raw| parse_cmdline(&raw))
        .unwrap_or(MetricValue::Unavailable {
            reason: "cmdline unreadable".to_string(),
        });

    let cgroup_path = source
        .read(&format!("proc/{pid}/cgroup"))
        .ok()
        .and_then(|raw| super::parse_cgroup_v2_path(&raw))
        .unwrap_or_default();

    let category = classify_process(&ProcessClassifyInput {
        comm: &stat.comm,
        exe_path: None,
        uid: owner_uid,
        cgroup_path: &cgroup_path,
        cmdline_is_empty,
    });

    let cpu_ticks = stat.utime + stat.stime;
    let cpu_percent = match prev {
        Some(p) => {
            let ticks_per_sec = rate_per_second(p.cpu_ticks, cpu_ticks, ctx);
            match ticks_per_sec {
                MetricValue::Supported { value } => MetricValue::Supported {
                    value: value / USER_HZ * 100.0,
                },
                other => other,
            }
        }
        None => MetricValue::Unavailable {
            reason: "insufficient samples yet".to_string(),
        },
    };

    let (disk_io_capability, disk_read_bytes_per_sec, disk_write_bytes_per_sec, disk_read_bytes, disk_write_bytes) =
        match source.read(&format!("proc/{pid}/io")) {
            Ok(raw) => {
                let (read_bytes, write_bytes) = parse_io_bytes(&raw);
                let (read_rate, write_rate) = match prev {
                    Some(p) => (
                        rate_per_second(p.disk_read_bytes, read_bytes, ctx),
                        rate_per_second(p.disk_write_bytes, write_bytes, ctx),
                    ),
                    None => (
                        MetricValue::Unavailable {
                            reason: "insufficient samples yet".to_string(),
                        },
                        MetricValue::Unavailable {
                            reason: "insufficient samples yet".to_string(),
                        },
                    ),
                };
                (Capability::Supported, read_rate, write_rate, read_bytes, write_bytes)
            }
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => (
                Capability::PermissionRequired {
                    reason: "process not owned by this service; /proc/[pid]/io requires the same uid or root".to_string(),
                },
                MetricValue::Unavailable {
                    reason: "permission required".to_string(),
                },
                MetricValue::Unavailable {
                    reason: "permission required".to_string(),
                },
                0,
                0,
            ),
            Err(_) => (
                Capability::Unavailable {
                    reason: "no /proc/[pid]/io for this process (kernel thread or already exited)".to_string(),
                },
                MetricValue::Unavailable {
                    reason: "unavailable".to_string(),
                },
                MetricValue::Unavailable {
                    reason: "unavailable".to_string(),
                },
                0,
                0,
            ),
        };

    let info = ProcessInfo {
        pid,
        start_time_ticks: stat.starttime,
        comm: stat.comm,
        cmdline,
        owner_uid,
        cpu_percent,
        rss_bytes,
        category,
        disk_io_capability,
        disk_read_bytes_per_sec,
        disk_write_bytes_per_sec,
        network_io_capability: Capability::Unavailable {
            reason: "per-process network attribution requires eBPF/cgroup net-cls, not available via /proc on Linux".to_string(),
        },
        network_rx_bytes_per_sec: MetricValue::Unavailable {
            reason: "not available on Linux via /proc".to_string(),
        },
        network_tx_bytes_per_sec: MetricValue::Unavailable {
            reason: "not available on Linux via /proc".to_string(),
        },
    };

    let next_prev = PrevProcessCounters {
        cpu_ticks,
        disk_read_bytes,
        disk_write_bytes,
    };

    Some((info, next_prev))
}

pub fn parse_process_snapshot(
    source: &dyn ProcSource,
    ctx: &SampleContext,
    prev: Option<&PrevProcessState>,
) -> Result<(ProcessSnapshot, PrevProcessState), TelemetryError> {
    let listing = require(source, "proc/.listing")?;
    let mut next_prev = PrevProcessState::new();
    let mut processes = Vec::new();

    for name in listing.lines() {
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let prev_counters = prev.and_then(|p| p.get(&pid));
        if let Some((info, counters)) = parse_one_process(source, pid, ctx, prev_counters) {
            processes.push(info);
            next_prev.insert(pid, counters);
        }
    }

    let snapshot = ProcessSnapshot {
        timestamp: ctx.now,
        total_count: processes.len() as u32,
        processes,
    };

    Ok((snapshot, next_prev))
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

    /// SRS FR-PRO-001 (CPU%/RSS/owner/cmdline per process) and
    /// FR-PRO-002 (per-process disk/network I/O only where the platform
    /// supports it — the snapshot locks in `PermissionRequired`/
    /// `Unavailable`, never a fabricated value).
    #[test]
    fn idle() {
        let source = FixtureProcSource::scenario("idle");
        let (snapshot, _prev) = parse_process_snapshot(&source, &fixed_ctx(), None).unwrap();
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn five_hundred_processes() {
        let source = FixtureProcSource::scenario("500-processes");
        let (snapshot, _prev) = parse_process_snapshot(&source, &fixed_ctx(), None).unwrap();
        assert!(
            snapshot.total_count >= 500,
            "expected >= 500 processes, got {}",
            snapshot.total_count
        );
    }
}
