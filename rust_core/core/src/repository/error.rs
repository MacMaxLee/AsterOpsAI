#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("failed to open database at {path}: {source}")]
    Open {
        path: String,
        #[source]
        source: rusqlite::Error,
    },

    #[error("failed to create database directory {path}: {source}")]
    CreateDataDir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("migration failed: {0}")]
    MigrationFailed(String),

    #[error("database query failed: {0}")]
    Query(#[from] rusqlite::Error),

    #[error("failed to serialize audit detail: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("failed to check out a pooled connection: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("the repository writer task is not running")]
    WriterUnavailable,

    #[error("failed to spawn the database writer thread: {0}")]
    ThreadSpawn(#[from] std::io::Error),

    #[error("the repository writer task did not reply")]
    WriterDidNotReply,

    #[error("no action row with id {0}")]
    PolicyActionNotFound(i64),

    #[error("action {id} is not in the expected status: expected {expected:?}, actual {actual:?}")]
    PolicyActionWrongStatus {
        id: i64,
        expected: String,
        actual: String,
    },

    #[error("action {0}'s approval has expired")]
    PolicyActionExpired(i64),

    /// Unit U10 (SRS FR-TUNE-002): a second `tuning_plans` insert for a
    /// target that already has an `IN_FLIGHT` row hit the real partial
    /// `UNIQUE` index (migrations/V11) — a durable, race-free rejection,
    /// not an in-memory check.
    #[error("a tuning plan is already in flight for this target")]
    TuningPlanAlreadyInFlight,

    #[error("no tuning plan row with id {0}")]
    TuningPlanNotFound(i64),

    #[error("no security event row with id {0}")]
    SecurityEventNotFound(i64),

    #[error("no security incident row with id {0}")]
    SecurityIncidentNotFound(i64),
}
