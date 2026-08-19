use platform::linux::ProcSource;

use super::error::TelemetryError;

fn require(source: &dyn ProcSource, path: &str) -> Result<String, TelemetryError> {
    source.read(path).map_err(|source| TelemetryError::Io {
        path: path.to_string(),
        source,
    })
}

/// Core-internal candidate — deliberately has no `trusted` field. Trust
/// tracking needs memory across observations, which for unit U1 lives in
/// the service sampler's own in-process `HashSet` (ephemeral until unit U2
/// adds real persistence), one layer above this pure parser.
#[derive(Debug, Clone)]
pub struct DeviceCandidate {
    pub identifier: String,
    pub name: String,
    pub removable: bool,
}

pub fn parse_devices(source: &dyn ProcSource) -> Result<Vec<DeviceCandidate>, TelemetryError> {
    let listing = require(source, "sys/block/.listing")?;
    let mut devices = Vec::new();

    for name in listing.lines() {
        let removable = source
            .read(&format!("sys/block/{name}/removable"))
            .map(|raw| raw.trim() == "1")
            .unwrap_or(false);

        devices.push(DeviceCandidate {
            identifier: format!("block:{name}"),
            name: name.to_string(),
            removable,
        });
    }

    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::testing::FixtureProcSource;

    #[test]
    fn idle() {
        let source = FixtureProcSource::scenario("idle");
        let devices = parse_devices(&source).unwrap();
        assert!(!devices.is_empty());
    }
}
