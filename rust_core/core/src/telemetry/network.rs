use std::collections::HashMap;

use contracts::telemetry::{MetricValue, NetworkInterfaceInfo, NetworkSnapshot};
use platform::linux::ProcSource;

use super::context::SampleContext;
use super::error::TelemetryError;
use super::rate::rate_per_second;

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IfaceCounters {
    rx_bytes: u64,
    rx_packets: u64,
    rx_errors: u64,
    rx_drops: u64,
    tx_bytes: u64,
    tx_packets: u64,
    tx_errors: u64,
    tx_drops: u64,
}

pub type PrevNetState = HashMap<String, IfaceCounters>;

fn parse_net_dev(raw: &str) -> Vec<(String, IfaceCounters)> {
    let mut result = Vec::new();
    for line in raw.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let fields: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|f| f.parse().ok())
            .collect();
        if fields.len() < 16 {
            continue;
        }
        result.push((
            name.trim().to_string(),
            IfaceCounters {
                rx_bytes: fields[0],
                rx_packets: fields[1],
                rx_errors: fields[2],
                rx_drops: fields[3],
                tx_bytes: fields[8],
                tx_packets: fields[9],
                tx_errors: fields[10],
                tx_drops: fields[11],
            },
        ));
    }
    result
}

pub fn parse_network_snapshot(
    source: &dyn ProcSource,
    ctx: &SampleContext,
    prev: Option<&PrevNetState>,
) -> Result<(NetworkSnapshot, PrevNetState), TelemetryError> {
    let raw = require(source, "proc/net/dev")?;
    let parsed = parse_net_dev(&raw);

    let unavailable = || MetricValue::Unavailable {
        reason: "insufficient samples yet".to_string(),
    };

    let mut interfaces = Vec::new();
    let mut next_prev = PrevNetState::new();
    for (name, curr) in parsed {
        let prev_counters = prev.and_then(|p| p.get(&name));
        let info = match prev_counters {
            Some(p) => NetworkInterfaceInfo {
                name: name.clone(),
                rx_bytes_per_sec: rate_per_second(p.rx_bytes, curr.rx_bytes, ctx),
                tx_bytes_per_sec: rate_per_second(p.tx_bytes, curr.tx_bytes, ctx),
                rx_packets_per_sec: rate_per_second(p.rx_packets, curr.rx_packets, ctx),
                tx_packets_per_sec: rate_per_second(p.tx_packets, curr.tx_packets, ctx),
                rx_errors_per_sec: rate_per_second(p.rx_errors, curr.rx_errors, ctx),
                tx_errors_per_sec: rate_per_second(p.tx_errors, curr.tx_errors, ctx),
                rx_drops_per_sec: rate_per_second(p.rx_drops, curr.rx_drops, ctx),
                tx_drops_per_sec: rate_per_second(p.tx_drops, curr.tx_drops, ctx),
            },
            None => NetworkInterfaceInfo {
                name: name.clone(),
                rx_bytes_per_sec: unavailable(),
                tx_bytes_per_sec: unavailable(),
                rx_packets_per_sec: unavailable(),
                tx_packets_per_sec: unavailable(),
                rx_errors_per_sec: unavailable(),
                tx_errors_per_sec: unavailable(),
                rx_drops_per_sec: unavailable(),
                tx_drops_per_sec: unavailable(),
            },
        };
        interfaces.push(info);
        next_prev.insert(name, curr);
    }

    let snapshot = NetworkSnapshot {
        timestamp: ctx.now,
        interfaces,
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

    #[test]
    fn idle() {
        let source = FixtureProcSource::scenario("idle");
        let (snapshot, _prev) = parse_network_snapshot(&source, &fixed_ctx(), None).unwrap();
        insta::assert_json_snapshot!(snapshot);
    }

    #[test]
    fn nic_counter_reset() {
        let before = FixtureProcSource::scenario_phase("nic-counter-reset", "before");
        let after = FixtureProcSource::scenario_phase("nic-counter-reset", "after");
        let (_, prev) = parse_network_snapshot(&before, &fixed_ctx(), None).unwrap();
        let (snapshot, _) = parse_network_snapshot(&after, &fixed_ctx(), Some(&prev)).unwrap();
        let enp0s5 = snapshot
            .interfaces
            .iter()
            .find(|i| i.name == "enp0s5")
            .unwrap();
        assert!(matches!(
            enp0s5.rx_bytes_per_sec,
            MetricValue::CounterReset { .. }
        ));
        insta::assert_json_snapshot!(snapshot);
    }
}
