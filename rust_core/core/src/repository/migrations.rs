use rusqlite::Connection;

use super::error::RepositoryError;

// Path is relative to this crate's CARGO_MANIFEST_DIR (rust_core/core/), so
// "../../migrations" resolves to the workspace-root /migrations/ directory.
refinery::embed_migrations!("../../migrations");

/// Runs any pending migrations against `conn`. Refinery only ever executes
/// `CREATE`/`ALTER` statements inside its own tracked transaction — a
/// failure rolls back cleanly under WAL and never drops or recreates the
/// database file (requirement 3). Idempotent: re-running against an
/// already-migrated database is a no-op.
pub fn run(conn: &mut Connection) -> Result<(), RepositoryError> {
    migrations::runner()
        .run(conn)
        .map_err(|err| RepositoryError::MigrationFailed(err.to_string()))?;
    Ok(())
}
