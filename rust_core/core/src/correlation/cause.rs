//! SRS FR-CORR-002's exact minimum list. Not every `analysis::
//! DbHealthCategory`/`HostBottleneck` variant maps to one of these nine —
//! see `docs/adr/0017` for what's deliberately left unmapped.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootCause {
    DbLocks,
    DbConfiguration,
    ConnectionExhaustion,
    SlowSql,
    HostCpu,
    HostMemory,
    StorageLatency,
    Network,
    /// Unexplained by any of the other eight — see `correlate::
    /// client_side_confidence`'s own doc comment for how this is
    /// computed (never from its own direct signal; there isn't one).
    ClientSideApplication,
}

impl RootCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DbLocks => "DB_LOCKS",
            Self::DbConfiguration => "DB_CONFIGURATION",
            Self::ConnectionExhaustion => "CONNECTION_EXHAUSTION",
            Self::SlowSql => "SLOW_SQL",
            Self::HostCpu => "HOST_CPU",
            Self::HostMemory => "HOST_MEMORY",
            Self::StorageLatency => "STORAGE_LATENCY",
            Self::Network => "NETWORK",
            Self::ClientSideApplication => "CLIENT_SIDE_APPLICATION",
        }
    }

    /// The eight concrete causes correlation ever attaches direct
    /// evidence to — deliberately excludes `ClientSideApplication`,
    /// which is computed from these eight's own results, not one of
    /// them.
    pub const EVIDENCED: [Self; 8] = [
        Self::DbLocks,
        Self::DbConfiguration,
        Self::ConnectionExhaustion,
        Self::SlowSql,
        Self::HostCpu,
        Self::HostMemory,
        Self::StorageLatency,
        Self::Network,
    ];
}
