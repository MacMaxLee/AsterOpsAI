use crate::actions::ActionError;
use crate::policy::PolicyError;
use crate::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum TuningError {
    /// FR-TUNE-002: a second `start_plan` call for a target that already
    /// has an `IN_FLIGHT` plan is rejected outright, not queued — mapped
    /// from `RepositoryError::TuningPlanAlreadyInFlight`'s real partial
    /// `UNIQUE` index violation (migrations/V11).
    #[error("a tuning plan is already in flight for this target")]
    PlanAlreadyInFlight,

    #[error("tuning against a DbSession target is not supported")]
    UnsupportedTarget,

    #[error("no tuning plan with id {0}")]
    PlanNotFound(i64),

    #[error("failed to read live process state: {0}")]
    CapabilityUnavailable(String),

    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),

    #[error("action error: {0}")]
    Action(#[from] ActionError),

    #[error("repository error: {0}")]
    Repository(RepositoryError),

    #[error("failed to serialize/deserialize: {0}")]
    Serde(#[from] serde_json::Error),
}

impl From<RepositoryError> for TuningError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::TuningPlanAlreadyInFlight => Self::PlanAlreadyInFlight,
            RepositoryError::TuningPlanNotFound(id) => Self::PlanNotFound(id),
            other => Self::Repository(other),
        }
    }
}
