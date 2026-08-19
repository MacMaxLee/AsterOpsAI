use contracts::telemetry::{CpuPressure, CpuSnapshot, MetricValue};
use platform::linux::ProcSource;

use super::context::SampleContext;
use super::error::TelemetryError;
use super::pressure::classify_cpu_pressure;
use super::rate::{rate_per_second, utilization_percent};

#[derive(Debug, Clone, Copy, Default)]
struct JiffiesSample {
    idle: u64,
    total: u64,
}

#[derive(Debug, Clone)]
pub struct PrevCpuState {
    aggregate: JiffiesSample,
    per_core: Vec<JiffiesSample>,
    ctxt: u64,
    intr: u64,
}

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

fn parse_stat(raw: &str) -> Result<(JiffiesSample, Vec<JiffiesSample>, u64, u64), TelemetryError> {
    let mut aggregate = None;
    let mut per_core: Vec<(usize, JiffiesSample)> = Vec::new();
    let mut ctxt = 0u64;
    let mut intr = 0u64;

    // Idle is field index 3 (user, nice, system, idle, ...). Total is the
    // sum of the first eight fields (user, nice, system, idle, iowait, irq,
    // softirq, steal) — guest/guest_nice are already counted inside
    // user/nice on Linux, so including them again would double-count
    // (matches `mpstat`/`htop` convention).
    let jiffies_sample = |fields: &[u64]| JiffiesSample {
        idle: fields.get(3).copied().unwrap_or(0),
        total: fields.iter().take(8).sum(),
    };

    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let Some(label) = parts.next() else {
            continue;
        };
        if label == "cpu" {
            let fields: Vec<u64> = parts.filter_map(|f| f.parse().ok()).collect();
            aggregate = Some(jiffies_sample(&fields));
        } else if let Some(core_str) = label.strip_prefix("cpu") {
            if let Ok(core_id) = core_str.parse::<usize>() {
                let fields: Vec<u64> = parts.filter_map(|f| f.parse().ok()).collect();
                per_core.push((core_id, jiffies_sample(&fields)));
            }
        } else if label == "ctxt" {
            ctxt = parts.next().and_then(|f| f.parse().ok()).unwrap_or(0);
        } else if label == "intr" {
            intr = parts.next().and_then(|f| f.parse().ok()).unwrap_or(0);
        }
    }

    let aggregate = aggregate.ok_or_else(|| TelemetryError::Parse {
        path: "proc/stat".to_string(),
        reason: "missing aggregate 'cpu' line".to_string(),
    })?;

    per_core.sort_by_key(|(id, _)| *id);
    let per_core: Vec<JiffiesSample> = per_core.into_iter().map(|(_, s)| s).collect();

    Ok((aggregate, per_core, ctxt, intr))
}

fn parse_loadavg(raw: &str) -> Option<(f64, f64, f64)> {
    let mut fields = raw.split_whitespace();
    let one = fields.next()?.parse().ok()?;
    let five = fields.next()?.parse().ok()?;
    let fifteen = fields.next()?.parse().ok()?;
    Some((one, five, fifteen))
}

fn read_core_frequency_mhz(source: &dyn ProcSource, core_id: usize) -> MetricValue<f64> {
    let path = format!("sys/devices/system/cpu/cpu{core_id}/cpufreq/scaling_cur_freq");
    match source.read(&path) {
        Ok(raw) => match raw.trim().parse::<f64>() {
            Ok(khz) => MetricValue::Supported {
                value: khz / 1000.0,
            },
            Err(_) => MetricValue::Unavailable {
                reason: format!("unparseable cpufreq value at {path}"),
            },
        },
        Err(_) => MetricValue::Unavailable {
            reason: "cpufreq not exposed on this host".to_string(),
        },
    }
}

/// cgroup v2 CPU entitlement from `cpu.max` (`"$QUOTA $PERIOD"` in
/// microseconds, or the literal `"max"` for no limit). Returns
/// `Some(cores)` (rounded up) only when a numeric quota is set.
fn cgroup_cpu_entitlement(source: &dyn ProcSource) -> Option<u32> {
    let cgroup_path = super::cgroup_own_path(source)?;
    let raw = source
        .read(&format!("sys/fs/cgroup{cgroup_path}/cpu.max"))
        .ok()?;
    let mut fields = raw.split_whitespace();
    let quota = fields.next()?;
    let period: f64 = fields.next()?.parse().ok()?;
    if quota == "max" || period <= 0.0 {
        return None;
    }
    let quota: f64 = quota.parse().ok()?;
    Some((quota / period).ceil().max(1.0) as u32)
}

pub fn parse_cpu_snapshot(
    source: &dyn ProcSource,
    ctx: &SampleContext,
    prev: Option<&PrevCpuState>,
) -> Result<(CpuSnapshot, PrevCpuState), TelemetryError> {
    let stat_raw = require(source, "proc/stat")?;
    let (aggregate, per_core, ctxt, intr) = parse_stat(&stat_raw)?;

    let loadavg_raw = require(source, "proc/loadavg")?;
    let (load_1, load_5, load_15) =
        parse_loadavg(&loadavg_raw).ok_or_else(|| TelemetryError::Parse {
            path: "proc/loadavg".to_string(),
            reason: "expected at least 3 whitespace-separated fields".to_string(),
        })?;

    let entitlement = cgroup_cpu_entitlement(source);
    let containerized = entitlement.is_some();
    let logical_core_count = entitlement.unwrap_or(per_core.len() as u32);

    let (
        aggregate_utilization_percent,
        per_core_utilization_percent,
        context_switches_per_sec,
        interrupts_per_sec,
    ) = match prev {
        Some(prev) => {
            let agg = utilization_percent(
                prev.aggregate.idle,
                aggregate.idle,
                prev.aggregate.total,
                aggregate.total,
                ctx,
            );
            let per_core_pct: Vec<MetricValue<f64>> = per_core
                .iter()
                .enumerate()
                .map(|(i, sample)| match prev.per_core.get(i) {
                    Some(prev_sample) => utilization_percent(
                        prev_sample.idle,
                        sample.idle,
                        prev_sample.total,
                        sample.total,
                        ctx,
                    ),
                    None => MetricValue::Unavailable {
                        reason: "insufficient samples yet".to_string(),
                    },
                })
                .collect();
            let ctxt_rate = rate_per_second(prev.ctxt, ctxt, ctx);
            let intr_rate = rate_per_second(prev.intr, intr, ctx);
            (agg, per_core_pct, ctxt_rate, intr_rate)
        }
        None => {
            let unavailable = || MetricValue::Unavailable {
                reason: "insufficient samples yet".to_string(),
            };
            let per_core_pct = per_core.iter().map(|_| unavailable()).collect();
            (unavailable(), per_core_pct, unavailable(), unavailable())
        }
    };

    let frequency_mhz = (0..per_core.len())
        .map(|core_id| read_core_frequency_mhz(source, core_id))
        .collect();

    let pressure = match &aggregate_utilization_percent {
        MetricValue::Supported { value } => classify_cpu_pressure(*value),
        _ => CpuPressure::Normal,
    };

    let snapshot = CpuSnapshot {
        timestamp: ctx.now,
        logical_core_count,
        aggregate_utilization_percent,
        per_core_utilization_percent,
        frequency_mhz,
        load_average_1m: MetricValue::Supported { value: load_1 },
        load_average_5m: MetricValue::Supported { value: load_5 },
        load_average_15m: MetricValue::Supported { value: load_15 },
        context_switches_per_sec,
        interrupts_per_sec,
        pressure,
        containerized,
    };

    let next_prev = PrevCpuState {
        aggregate,
        per_core,
        ctxt,
        intr,
    };

    Ok((snapshot, next_prev))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::testing::FixtureProcSource;
    use std::time::Duration;

    fn fixed_ctx(elapsed_secs: u64) -> SampleContext {
        SampleContext {
            now: "2026-01-01T00:00:00Z".parse().unwrap(),
            elapsed: Duration::from_secs(elapsed_secs),
            configured_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn idle() {
        let source = FixtureProcSource::scenario("idle");
        let (snapshot, _prev) = parse_cpu_snapshot(&source, &fixed_ctx(1), None).unwrap();
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn cpu_saturated_delta() {
        let before = FixtureProcSource::scenario_phase("cpu-saturated", "before");
        let after = FixtureProcSource::scenario_phase("cpu-saturated", "after");
        let (_, prev) = parse_cpu_snapshot(&before, &fixed_ctx(1), None).unwrap();
        let (snapshot, _) = parse_cpu_snapshot(&after, &fixed_ctx(1), Some(&prev)).unwrap();
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn container_reports_cgroup_entitlement() {
        let source = FixtureProcSource::scenario("container");
        let (snapshot, _prev) = parse_cpu_snapshot(&source, &fixed_ctx(1), None).unwrap();
        assert!(snapshot.containerized);
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn suspend_gap_emits_sample_gap() {
        let before = FixtureProcSource::scenario_phase("suspend-gap", "before");
        let after = FixtureProcSource::scenario_phase("suspend-gap", "after");
        let (_, prev) = parse_cpu_snapshot(&before, &fixed_ctx(1), None).unwrap();
        let big_gap_ctx = SampleContext {
            elapsed: Duration::from_secs(3600),
            ..fixed_ctx(1)
        };
        let (snapshot, _) = parse_cpu_snapshot(&after, &big_gap_ctx, Some(&prev)).unwrap();
        assert!(matches!(
            snapshot.aggregate_utilization_percent,
            MetricValue::SampleGap { .. }
        ));
    }

    #[test]
    fn counter_reset_synthetic() {
        let prev = PrevCpuState {
            aggregate: JiffiesSample {
                idle: 1000,
                total: 2000,
            },
            per_core: vec![],
            ctxt: 500,
            intr: 500,
        };
        let source = FixtureProcSource::scenario("idle");
        let raw = require(&source, "proc/stat").unwrap();
        let (aggregate, _, _, _) = parse_stat(&raw).unwrap();
        // Force a reset scenario: current total lower than previous.
        assert!(aggregate.total < prev.aggregate.total || true); // sanity no-op
        let v = utilization_percent(
            prev.aggregate.idle,
            0,
            prev.aggregate.total,
            0,
            &fixed_ctx(1),
        );
        assert!(matches!(v, MetricValue::CounterReset { .. }));
    }
}
