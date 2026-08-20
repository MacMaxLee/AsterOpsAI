use crate::policy::PolicyError;
use crate::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    /// TRS §27: a target-identity re-verification, immediately before
    /// apply or immediately before rollback, found the live target no
    /// longer matches what was approved — abort, never act on a reused
    /// identifier.
    #[error("target identity changed since approval")]
    TargetChanged,

    #[error("action capability unavailable: {0}")]
    CapabilityUnavailable(String),

    #[error("resource is protected: {0}")]
    Protected(String),

    /// Unit U11 (SRS FR-SEC-003): a security-relevant flag (an open
    /// incident) on this resource overrides a performance-motivated
    /// action targeting it — a security response action itself is never
    /// blocked by its own resource's flag (`ActionKind::
    /// is_security_response`).
    #[error("resource has an open security incident: {0}")]
    SecurityFlagged(String),

    #[error("failed to capture previous state: {0}")]
    CapturePreviousState(String),

    #[error("action apply failed: {0}")]
    Apply(String),

    #[error("action verify failed: {0}")]
    Verify(String),

    #[error("action rollback failed: {0}")]
    Rollback(String),

    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("failed to serialize/deserialize: {0}")]
    Serde(#[from] serde_json::Error),
}
