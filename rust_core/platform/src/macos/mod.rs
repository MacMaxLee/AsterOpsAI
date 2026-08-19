pub mod exec;

use crate::{adapter::ProcessSelfMetrics, error::CapabilityError, PlatformAdapter};

pub struct MacosPlatformAdapter;

impl PlatformAdapter for MacosPlatformAdapter {
    fn platform_name(&self) -> &'static str {
        "macos"
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "macos self-process metrics not implemented yet, see unit U12".to_string(),
        ))
    }
}
