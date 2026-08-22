use contracts::telemetry::{MemorySnapshot, MetricValue, NumaNodeMemory};
use platform::linux::ProcSource;

use super::context::SampleContext;
use super::error::TelemetryError;
use super::pressure::classify_memory_pressure;

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

/// `/proc/meminfo` is `"Key:   value kB"` (or occasionally no unit, treated
/// as bytes) — collects every key the parser cares about in one pass.
fn parse_meminfo_kb(raw: &str) -> std::collections::HashMap<&str, u64> {
    let mut map = std::collections::HashMap::new();
    for line in raw.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value_kb = rest
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<u64>().ok());
        if let Some(value_kb) = value_kb {
            map.insert(key, value_kb);
        }
    }
    map
}

fn cgroup_memory(source: &dyn ProcSource) -> Option<(u64, u64)> {
    let cgroup_path = super::cgroup_own_path(source)?;
    let max_raw = source
        .read(&format!("sys/fs/cgroup{cgroup_path}/memory.max"))
        .ok()?;
    let max_trimmed = max_raw.trim();
    if max_trimmed == "max" {
        return None;
    }
    let max: u64 = max_trimmed.parse().ok()?;
    let current_raw = source
        .read(&format!("sys/fs/cgroup{cgroup_path}/memory.current"))
        .ok()?;
    let current: u64 = current_raw.trim().parse().ok()?;
    Some((max, current))
}

fn parse_numa_nodes(source: &dyn ProcSource) -> MetricValue<Vec<NumaNodeMemory>> {
    let listing = match source.read("sys/devices/system/node/.listing") {
        Ok(listing) => listing,
        Err(_) => {
            return MetricValue::Unavailable {
                reason: "NUMA node topology not exposed on this host".to_string(),
            }
        }
    };

    let mut nodes = Vec::new();
    for name in listing.lines() {
        let Some(id_str) = name.strip_prefix("node") else {
            continue;
        };
        let Ok(node_id) = id_str.parse::<u32>() else {
            continue;
        };
        let Ok(raw) = source.read(&format!("sys/devices/system/node/{name}/meminfo")) else {
            continue;
        };
        let mut total_bytes = 0u64;
        let mut free_bytes = 0u64;
        for line in raw.lines() {
            // "Node 0 MemTotal:        4046652 kB"
            if let Some(rest) = line.split("MemTotal:").nth(1) {
                total_bytes = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0)
                    * 1024;
            } else if let Some(rest) = line.split("MemFree:").nth(1) {
                free_bytes = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0)
                    * 1024;
            }
        }
        nodes.push(NumaNodeMemory {
            node_id,
            total_bytes,
            free_bytes,
        });
    }

    if nodes.is_empty() {
        MetricValue::Unavailable {
            reason: "no NUMA nodes enumerated".to_string(),
        }
    } else {
        MetricValue::Supported { value: nodes }
    }
}

pub fn parse_memory_snapshot(
    source: &dyn ProcSource,
    ctx: &SampleContext,
) -> Result<MemorySnapshot, TelemetryError> {
    let raw = require(source, "proc/meminfo")?;
    let fields = parse_meminfo_kb(&raw);
    let kb = |key: &str| fields.get(key).copied().unwrap_or(0) * 1024;

    let host_total = kb("MemTotal");
    let host_available = kb("MemAvailable");
    let cached_bytes = kb("Cached");
    let buffers_bytes = kb("Buffers");
    // Known simplification: always the host's swap, even when containerized.
    // cgroup v2 has its own per-container swap accounting
    // (memory.swap.current/memory.swap.max), but reading it is out of scope
    // for this unit; pressure classification below is a best-effort mix of
    // cgroup-scoped memory and host-scoped swap until that's added.
    let swap_total_bytes = kb("SwapTotal");
    let swap_free_bytes = kb("SwapFree");
    let swap_used_bytes = swap_total_bytes.saturating_sub(swap_free_bytes);

    let cgroup = cgroup_memory(source);
    let containerized = cgroup.is_some();
    let (total_bytes, used_bytes, available_bytes) = match cgroup {
        Some((max, current)) => (max, current, max.saturating_sub(current)),
        None => (
            host_total,
            host_total.saturating_sub(host_available),
            host_available,
        ),
    };

    let pressure = classify_memory_pressure(
        total_bytes,
        available_bytes,
        swap_total_bytes,
        swap_used_bytes,
    );
    let numa_nodes = parse_numa_nodes(source);

    Ok(MemorySnapshot {
        timestamp: ctx.now,
        total_bytes: MetricValue::Supported { value: total_bytes },
        used_bytes: MetricValue::Supported { value: used_bytes },
        available_bytes: MetricValue::Supported {
            value: available_bytes,
        },
        cached_bytes: MetricValue::Supported {
            value: cached_bytes,
        },
        buffers_bytes: MetricValue::Supported {
            value: buffers_bytes,
        },
        swap_total_bytes: MetricValue::Supported {
            value: swap_total_bytes,
        },
        swap_used_bytes: MetricValue::Supported {
            value: swap_used_bytes,
        },
        swap_free_bytes: MetricValue::Supported {
            value: swap_free_bytes,
        },
        pressure,
        containerized,
        numa_nodes,
    })
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
        let snapshot = parse_memory_snapshot(&source, &fixed_ctx()).unwrap();
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn swapping() {
        let source = FixtureProcSource::scenario("swapping");
        let snapshot = parse_memory_snapshot(&source, &fixed_ctx()).unwrap();
        assert!(matches!(
            snapshot.swap_used_bytes,
            MetricValue::Supported { value } if value > 0
        ));
        insta::assert_json_snapshot!(snapshot);
    }

    /// SRS FR-MEM-002: inside a container, the cgroup limit is the
    /// reported ceiling, not the host's own total.
    #[test]
    fn container_reports_cgroup_limit() {
        let source = FixtureProcSource::scenario("container");
        let snapshot = parse_memory_snapshot(&source, &fixed_ctx()).unwrap();
        assert!(snapshot.containerized);
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn numa_2node() {
        let source = FixtureProcSource::scenario("numa-2node");
        let snapshot = parse_memory_snapshot(&source, &fixed_ctx()).unwrap();
        assert!(matches!(
            &snapshot.numa_nodes,
            MetricValue::Supported { value } if value.len() == 2
        ));
        insta::assert_json_snapshot!(snapshot);
    }
}
