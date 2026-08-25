//! macOS Network telemetry via `netstat -ibn`.
//!
//! Collects per-interface network statistics using:
//! - `netstat -ibn` to get interface bytes/packets/errors
//! - Rate calculations from counter deltas over time
//!
//! Mirrors `telemetry/network.rs` structure but uses macOS-specific commands.
//! Established in unit U99.

use std::collections::HashMap;

use contracts::telemetry::{MetricValue, NetworkInterfaceInfo, NetworkSnapshot};

use super::context::SampleContext;
use super::error::TelemetryError;
use super::rate::rate_per_second;

// ============================================================================
// State Tracking Structures
// ============================================================================

#[derive(Debug, Clone, Copy, Default)]
pub struct IfaceCounters {
    rx_bytes: u64,
    rx_packets: u64,
    rx_errors: u64,
    tx_bytes: u64,
    tx_packets: u64,
    tx_errors: u64,
}

pub type PrevNetState = HashMap<String, IfaceCounters>;

// ============================================================================
// Helper Functions
// ============================================================================

/// Parses `netstat -ibn` output to extract interface statistics.
///
/// Expected format:
/// ```
/// Name  Mtu   Network       Address            Ipkts Ierrs    Ibytes Opkts Oerrs    Obytes  Coll
/// lo0   16384 <Link#1>                      12345678     0 987654321 12345678     0 987654321     0
/// en0   1500  <Link#2>      xx:xx:xx:xx:xx:xx 23456789    10 234567890 34567890    20 345678901     0
/// ```
fn parse_netstat_ibn(raw: &str) -> Vec<(String, IfaceCounters)> {
    let mut result = Vec::new();

    for line in raw.lines().skip(1) {
        // Skip header line
        let fields: Vec<&str> = line.split_whitespace().collect();

        // Need at least: Name Mtu Network Ipkts Ierrs Ibytes Opkts Oerrs Obytes
        // (Address field may be present or absent)
        if fields.len() < 9 {
            continue;
        }

        let name = fields[0];

        // Parse numeric fields (Ipkts Ierrs Ibytes Opkts Oerrs Obytes)
        let parse_u64 = |idx: usize| -> u64 {
            fields.get(idx)
                .and_then(|f| f.parse().ok())
                .unwrap_or(0)
        };

        // Detect if Address field is present by checking if field[3] is numeric
        // If field[3] parses as a number, it's Ipkts (no Address)
        // If field[3] doesn't parse as a number, it's Address, so Ipkts is at field[4]
        let has_address = fields.get(3)
            .and_then(|f| f.parse::<u64>().ok())
            .is_none();

        let offset = if has_address { 1 } else { 0 };

        let rx_packets = parse_u64(3 + offset);  // Ipkts
        let rx_errors = parse_u64(4 + offset);   // Ierrs
        let rx_bytes = parse_u64(5 + offset);    // Ibytes
        let tx_packets = parse_u64(6 + offset);  // Opkts
        let tx_errors = parse_u64(7 + offset);   // Oerrs
        let tx_bytes = parse_u64(8 + offset);    // Obytes

        result.push((
            name.to_string(),
            IfaceCounters {
                rx_bytes,
                rx_packets,
                rx_errors,
                tx_bytes,
                tx_packets,
                tx_errors,
            },
        ));
    }

    result
}

/// Gets network interface statistics via netstat command.
/// Uses platform::macos::exec::get_netstat_interfaces() to execute the command.
fn get_netstat_data() -> Result<String, TelemetryError> {
    platform::macos::exec::get_netstat_interfaces().map_err(|e| TelemetryError::Io {
        path: "netstat -ibn".to_string(),
        source: e,
    })
}

// ============================================================================
// Main Parser Function
// ============================================================================

/// Parses a network snapshot on macOS using netstat.
///
/// Returns `(NetworkSnapshot, PrevNetState)` where the second element is the
/// state to pass to the next invocation for delta calculations.
///
/// On the first sample (`prev == None`), all rate metrics return
/// `MetricValue::Unavailable` since there's no baseline for deltas.
pub fn parse_network_snapshot(
    ctx: &SampleContext,
    prev: Option<&PrevNetState>,
) -> Result<(NetworkSnapshot, PrevNetState), TelemetryError> {
    let raw = get_netstat_data()?;
    let parsed = parse_netstat_ibn(&raw);

    let unavailable = || MetricValue::Unavailable {
        reason: "insufficient samples yet".to_string(),
    };

    let mut interfaces = Vec::new();
    let mut next_prev = PrevNetState::new();

    for (name, curr) in parsed {
        // Filter out loopback interface (lo0 on macOS)
        if name.starts_with("lo") {
            continue;
        }

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
                // macOS netstat doesn't expose drop counters directly
                rx_drops_per_sec: MetricValue::Unavailable {
                    reason: "macOS netstat does not expose drop counters".to_string(),
                },
                tx_drops_per_sec: MetricValue::Unavailable {
                    reason: "macOS netstat does not expose drop counters".to_string(),
                },
            },
            None => NetworkInterfaceInfo {
                name: name.clone(),
                rx_bytes_per_sec: unavailable(),
                tx_bytes_per_sec: unavailable(),
                rx_packets_per_sec: unavailable(),
                tx_packets_per_sec: unavailable(),
                rx_errors_per_sec: unavailable(),
                tx_errors_per_sec: unavailable(),
                rx_drops_per_sec: MetricValue::Unavailable {
                    reason: "macOS netstat does not expose drop counters".to_string(),
                },
                tx_drops_per_sec: MetricValue::Unavailable {
                    reason: "macOS netstat does not expose drop counters".to_string(),
                },
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

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn fixed_ctx() -> SampleContext {
        SampleContext {
            now: chrono::Utc::now(),
            elapsed: Duration::from_secs(1),
            configured_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn network_snapshot_returns_interfaces() {
        let (snapshot, _prev) = parse_network_snapshot(&fixed_ctx(), None)
            .expect("network snapshot should succeed");

        // Should have at least one non-loopback interface
        // (Could be 0 if running in unusual environment, but typically en0 exists)
        assert!(
            !snapshot.interfaces.is_empty(),
            "should have at least one network interface (typically en0)"
        );
    }

    #[test]
    fn first_sample_returns_unavailable_for_rates() {
        let (snapshot, _prev) = parse_network_snapshot(&fixed_ctx(), None)
            .expect("network snapshot should succeed");

        // All rate metrics should be unavailable on first sample
        for iface in &snapshot.interfaces {
            assert!(
                matches!(iface.rx_bytes_per_sec, MetricValue::Unavailable { .. }),
                "rx_bytes_per_sec should be unavailable on first sample for {}",
                iface.name
            );
            assert!(
                matches!(iface.tx_bytes_per_sec, MetricValue::Unavailable { .. }),
                "tx_bytes_per_sec should be unavailable on first sample for {}",
                iface.name
            );
        }
    }

    #[test]
    fn second_sample_calculates_rates() {
        let (_, prev) = parse_network_snapshot(&fixed_ctx(), None)
            .expect("first sample should succeed");

        // Wait a bit
        std::thread::sleep(Duration::from_millis(100));

        let (snapshot, _) = parse_network_snapshot(&fixed_ctx(), Some(&prev))
            .expect("second sample should succeed");

        // At least one interface should have supported rate metrics
        let has_supported = snapshot.interfaces.iter().any(|iface| {
            matches!(iface.rx_bytes_per_sec, MetricValue::Supported { .. })
                || matches!(iface.tx_bytes_per_sec, MetricValue::Supported { .. })
        });

        assert!(
            has_supported,
            "at least one interface should have supported rate metrics on second sample"
        );
    }

    #[test]
    fn loopback_filtered_out() {
        let (snapshot, _) = parse_network_snapshot(&fixed_ctx(), None)
            .expect("network snapshot should succeed");

        // Verify no loopback interfaces (lo0, lo1, etc.)
        for iface in &snapshot.interfaces {
            assert!(
                !iface.name.starts_with("lo"),
                "loopback interface should be filtered: {}",
                iface.name
            );
        }
    }

    #[test]
    fn drop_counters_unavailable() {
        let (snapshot, _) = parse_network_snapshot(&fixed_ctx(), None)
            .expect("network snapshot should succeed");

        // All interfaces should have drop counters marked as unavailable
        for iface in &snapshot.interfaces {
            assert!(
                matches!(iface.rx_drops_per_sec, MetricValue::Unavailable { .. }),
                "rx_drops should be unavailable on macOS for {}",
                iface.name
            );
            assert!(
                matches!(iface.tx_drops_per_sec, MetricValue::Unavailable { .. }),
                "tx_drops should be unavailable on macOS for {}",
                iface.name
            );
        }
    }

    #[test]
    fn parse_netstat_output() {
        // Sample netstat -ibn output
        let sample = r#"Name  Mtu   Network       Address            Ipkts Ierrs    Ibytes Opkts Oerrs    Obytes  Coll
lo0   16384 <Link#1>                      1234567     0 987654321 1234567     0 987654321     0
en0   1500  <Link#2>      aa:bb:cc:dd:ee:ff 2345678    10 234567890 3456789    20 345678901     0
en1   1500  <Link#3>      11:22:33:44:55:66  345678     5  34567890  456789     8  45678901     0"#;

        let parsed = parse_netstat_ibn(sample);

        // Should parse 3 interfaces
        assert_eq!(parsed.len(), 3, "should parse 3 interfaces");

        // Check lo0
        let lo0 = parsed.iter().find(|(name, _)| name == "lo0").unwrap();
        assert_eq!(lo0.1.rx_packets, 1234567);
        assert_eq!(lo0.1.rx_bytes, 987654321);
        assert_eq!(lo0.1.tx_packets, 1234567);
        assert_eq!(lo0.1.tx_bytes, 987654321);

        // Check en0
        let en0 = parsed.iter().find(|(name, _)| name == "en0").unwrap();
        assert_eq!(en0.1.rx_packets, 2345678);
        assert_eq!(en0.1.rx_errors, 10);
        assert_eq!(en0.1.rx_bytes, 234567890);
        assert_eq!(en0.1.tx_packets, 3456789);
        assert_eq!(en0.1.tx_errors, 20);
        assert_eq!(en0.1.tx_bytes, 345678901);
    }
}
