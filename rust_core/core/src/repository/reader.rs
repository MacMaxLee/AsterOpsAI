use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;

use super::connection::ReadPool;
use super::error::RepositoryError;

/// Checks out one read-only connection from the pool. Safe to call
/// concurrently with the writer thread's own connection — both operate
/// under WAL mode, where readers never block the writer and vice versa.
pub fn checkout(
    pool: &ReadPool,
) -> Result<PooledConnection<SqliteConnectionManager>, RepositoryError> {
    pool.get().map_err(RepositoryError::from)
}
