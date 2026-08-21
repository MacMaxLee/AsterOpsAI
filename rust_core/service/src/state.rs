use std::sync::Arc;
use std::time::Instant;

use ai_ops_core::actions::ActionContext;
use ai_ops_core::dbms::DbmsAdapter;
use ai_ops_core::policy::{ActionTypeRegistry, ProtectedResourceRegistry};
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
    /// The shared dependency bundle `ActionTypeEntry::construct` needs
    /// (unit U22) — just a live `PlatformAdapter` handle today.
    pub action_context: ActionContext,
    /// Seeded once at startup (`main.rs`) with every real, shipped
    /// `ActionKind` — empty on non-Linux (unit U22: `core::actions::host`
    /// has no Windows/macOS implementation yet), in which case a grant
    /// attempt fails with a real, honest `UnknownActionType`, never a
    /// crash or a faked success.
    pub action_registry: Arc<ActionTypeRegistry>,
    /// FR-POL-006's protection stage runs for real on every `execute()`;
    /// this ships genuinely empty (unit U22) — no config surface for
    /// protecting specific resources exists anywhere in this project yet.
    pub protected_resources: Arc<ProtectedResourceRegistry>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        platform: Arc<dyn PlatformAdapter>,
        self_metrics: Arc<RwLock<SelfMetricsSnapshot>>,
        host_telemetry: Arc<RwLock<HostTelemetrySnapshot>>,
        repository: Option<RepositoryHandle>,
        dbms_adapter: Option<Arc<dyn DbmsAdapter>>,
        action_registry: Arc<ActionTypeRegistry>,
        protected_resources: Arc<ProtectedResourceRegistry>,
    ) -> Self {
        let action_context = ActionContext {
            platform: platform.clone(),
        };
        Self {
            started_at: Instant::now(),
            platform,
            self_metrics,
            host_telemetry,
            repository,
            dbms_adapter,
            action_context,
            action_registry,
            protected_resources,
        }
    }
}
