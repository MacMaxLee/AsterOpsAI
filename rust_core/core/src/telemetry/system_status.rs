use std::collections::BTreeMap;

use contracts::telemetry::{CpuPressure, MemoryPressure, SystemStatusResponse};
use contracts::{Capability, CapabilityFamily};
use platform::linux::ProcSource;

use super::context::SampleContext;
use super::error::TelemetryError;

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

fn parse_uptime_seconds(raw: &str) -> Option<u64> {
    let seconds: f64 = raw.split_whitespace().next()?.parse().ok()?;
    Some(seconds as u64)
}

#[allow(clippy::too_many_arguments)]
pub fn build_system_status_response(
    source: &dyn ProcSource,
    ctx: &SampleContext,
    containerized: bool,
    cpu_pressure: CpuPressure,
    memory_pressure: MemoryPressure,
    sample_interval_ms: u64,
    capabilities: BTreeMap<CapabilityFamily, Capability>,
) -> Result<SystemStatusResponse, TelemetryError> {
    let raw = require(source, "proc/uptime")?;
    let uptime_seconds = parse_uptime_seconds(&raw).ok_or_else(|| TelemetryError::Parse {
        path: "proc/uptime".to_string(),
        reason: "expected a numeric first field".to_string(),
    })?;

    Ok(SystemStatusResponse {
        timestamp: ctx.now,
        uptime_seconds,
        containerized,
        cpu_pressure,
        memory_pressure,
        sample_interval_ms,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::testing::FixtureProcSource;

    #[test]
    fn idle() {
        let source = FixtureProcSource::scenario("idle");
        let ctx = SampleContext {
            now: "2026-01-01T00:00:00Z".parse().unwrap(),
            elapsed: std::time::Duration::from_secs(1),
            configured_interval: std::time::Duration::from_secs(1),
        };
        let response = build_system_status_response(
            &source,
            &ctx,
            false,
            CpuPressure::Normal,
            MemoryPressure::Normal,
            1000,
            BTreeMap::new(),
        )
        .unwrap();
        assert!(response.uptime_seconds > 0);
    }
}
