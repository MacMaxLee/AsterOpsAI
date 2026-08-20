use crate::actions::ActionError;
use crate::policy::PolicyError;
use crate::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum BenchmarkError {
    #[error("failed to sample metric: {0}")]
    Sample(String),

    #[error("fewer than the configured minimum samples were collected: got {got}, need {need}")]
    InsufficientSamples { got: usize, need: usize },

    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),

    #[error("action error: {0}")]
    Action(#[from] ActionError),

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("failed to serialize/deserialize: {0}")]
    Serde(#[from] serde_json::Error),
}
