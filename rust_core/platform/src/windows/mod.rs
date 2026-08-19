pub mod exec;

use crate::{adapter::ProcessSelfMetrics, error::CapabilityError, PlatformAdapter};

pub struct WindowsPlatformAdapter;

impl PlatformAdapter for WindowsPlatformAdapter {
    fn platform_name(&self) -> &'static str {
        "windows"
    }

    fn self_process_metrics(&self) -> Result<ProcessSelfMetrics, CapabilityError> {
        Err(CapabilityError::Unsupported(
            "windows self-process metrics not implemented yet, see unit U12".to_string(),
        ))
    }
}
