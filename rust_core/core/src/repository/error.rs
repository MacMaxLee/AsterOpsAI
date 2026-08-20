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
}
