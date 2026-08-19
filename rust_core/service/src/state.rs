use std::sync::Arc;
use std::time::Instant;

use platform::PlatformAdapter;
use tokio::sync::RwLock;

use crate::self_metrics::SelfMetricsSnapshot;
use crate::telemetry::HostTelemetrySnapshot;

#[derive(Clone)]
pub struct AppState {
    pub started_at: Instant,
    pub platform: Arc<dyn PlatformAdapter>,
    pub self_metrics: Arc<RwLock<SelfMetricsSnapshot>>,
    pub host_telemetry: Arc<RwLock<HostTelemetrySnapshot>>,
}

impl AppState {
    pub fn new(
        platform: Arc<dyn PlatformAdapter>,
        self_metrics: Arc<RwLock<SelfMetricsSnapshot>>,
        host_telemetry: Arc<RwLock<HostTelemetrySnapshot>>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            platform,
            self_metrics,
            host_telemetry,
        }
    }
}
