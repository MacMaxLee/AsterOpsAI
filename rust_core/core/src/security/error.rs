#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),

    #[error("action error: {0}")]
    Action(#[from] crate::actions::ActionError),

    #[error("failed to serialize/deserialize: {0}")]
    Serde(#[from] serde_json::Error),
}
