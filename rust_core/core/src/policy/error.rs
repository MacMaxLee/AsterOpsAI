use crate::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("unknown action type: {0}")]
    UnknownActionType(String),

    #[error("invalid parameters for action type {action_type}: {reason}")]
    InvalidParameters { action_type: String, reason: String },

    #[error("action denied: {0}")]
    Denied(String),

    #[error("no action with id {0}")]
    NotFound(i64),

    #[error("action {id} is not in the expected status: expected {expected}, actual {actual}")]
    WrongStatus {
        id: i64,
        expected: String,
        actual: String,
    },

    #[error("approval for action {0} has expired")]
    Expired(i64),

    #[error("target or parameters do not match what was approved")]
    ApprovalMismatch,

    /// A real `construct` fn (unit U8) failed for a reason
    /// `validate_parameters` couldn't have caught — e.g. an action type
    /// registered for one `TargetIdentity` kind receiving another.
    #[error("failed to construct action: {0}")]
    ConstructionFailed(String),

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("failed to serialize/deserialize: {0}")]
    Serde(#[from] serde_json::Error),

    /// Unit U43: a fresh proposal for a target that already has an
    /// unresolved action of the same `action_type` — closes ADR 0034's
    /// own named "double-suspend" gap. See `RepositoryError::
    /// ConflictingActionInFlight`'s own doc comment for the full guard.
    #[error("a conflicting action is already in flight for this target")]
    ConflictingActionInFlight,
}

/// Preserves `RepositoryError`'s specific lifecycle-transition variants
/// (`NotFound`/`WrongStatus`/`Expired`) rather than flattening every
/// repository error into the generic `Repository` variant via `?` — those
/// three are exactly the states TRS §25 calls out as needing their own
/// dedicated, distinguishable rejection.
pub(super) fn map_transition_err(err: RepositoryError) -> PolicyError {
    match err {
        RepositoryError::PolicyActionNotFound(id) => PolicyError::NotFound(id),
        RepositoryError::PolicyActionWrongStatus {
            id,
            expected,
            actual,
        } => PolicyError::WrongStatus {
            id,
            expected,
            actual,
        },
        RepositoryError::PolicyActionExpired(id) => PolicyError::Expired(id),
        other => PolicyError::Repository(other),
    }
}

/// Unit U43: same discrimination discipline as `map_transition_err`,
/// for `propose_action`'s own call site (`evaluate()`) — preserves
/// `ConflictingActionInFlight` as its own distinguishable variant
/// rather than flattening it into the generic `Repository` case.
pub(super) fn map_propose_err(err: RepositoryError) -> PolicyError {
    match err {
        RepositoryError::ConflictingActionInFlight => PolicyError::ConflictingActionInFlight,
        other => PolicyError::Repository(other),
    }
}
