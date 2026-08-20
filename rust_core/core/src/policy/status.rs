//! `status` column values for the `actions` table (migrations/V4 + V9).
//! Stored/read as raw strings by `core::repository` (mirroring
//! `TelemetrySnapshotRow.cpu_pressure`'s own precedent) — this module owns
//! the actual state-machine semantics.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    AutoAllowed,
    PendingApproval,
    Approved,
    Denied,
    Executing,
    Executed,
    Failed,
    RolledBack,
}

impl ActionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoAllowed => "AUTO_ALLOWED",
            Self::PendingApproval => "PENDING_APPROVAL",
            Self::Approved => "APPROVED",
            Self::Denied => "DENIED",
            Self::Executing => "EXECUTING",
            Self::Executed => "EXECUTED",
            Self::Failed => "FAILED",
            Self::RolledBack => "ROLLED_BACK",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "AUTO_ALLOWED" => Some(Self::AutoAllowed),
            "PENDING_APPROVAL" => Some(Self::PendingApproval),
            "APPROVED" => Some(Self::Approved),
            "DENIED" => Some(Self::Denied),
            "EXECUTING" => Some(Self::Executing),
            "EXECUTED" => Some(Self::Executed),
            "FAILED" => Some(Self::Failed),
            "ROLLED_BACK" => Some(Self::RolledBack),
            _ => None,
        }
    }
}
