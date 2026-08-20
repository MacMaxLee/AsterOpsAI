use std::sync::Arc;
use std::time::Instant;

use ai_ops_core::dbms::DbmsAdapter;
use ai_ops_core::repository::RepositoryHandle;
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
    /// `None` when the repository layer failed to start (e.g. a migration
    /// error) — live telemetry keeps working regardless; only history
    /// endpoints degrade (requirement 3).
    pub repository: Option<RepositoryHandle>,
    /// `None` when no DB connection is configured (`dbms_config::
    /// resolve_db_connection`) or the initial connect failed — the
    /// correlation endpoint degrades to a real `unavailable_verdict` DB
    /// side rather than erroring (unit U20).
    pub dbms_adapter: Option<Arc<dyn DbmsAdapter>>,
}

impl AppState {
    pub fn new(
        platform: Arc<dyn PlatformAdapter>,
        self_metrics: Arc<RwLock<SelfMetricsSnapshot>>,
        host_telemetry: Arc<RwLock<HostTelemetrySnapshot>>,
        repository: Option<RepositoryHandle>,
        dbms_adapter: Option<Arc<dyn DbmsAdapter>>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            platform,
            self_metrics,
            host_telemetry,
            repository,
            dbms_adapter,
        }
    }
}
