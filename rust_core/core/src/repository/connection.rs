use std::path::Path;

use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OpenFlags};

use super::error::RepositoryError;

pub type ReadPool = r2d2::Pool<SqliteConnectionManager>;

const READ_POOL_SIZE: u32 = 4;

/// Applies the pragmas requirement 1 mandates. `auto_vacuum` is set
/// separately, only for a brand-new file, before any table exists — see
/// `open_write_connection`.
fn apply_pragmas(conn: &Connection) -> Result<(), RepositoryError> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

/// Opens the single write connection. If `path` doesn't exist yet, sets
/// `auto_vacuum = INCREMENTAL` before any table is created — changing
/// auto_vacuum mode on a populated database needs a full `VACUUM`, but on an
/// empty one it's instant.
pub fn open_write_connection(path: &Path) -> Result<Connection, RepositoryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| RepositoryError::CreateDataDir {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let is_new = !path.exists();
    let conn = Connection::open(path).map_err(|source| RepositoryError::Open {
        path: path.display().to_string(),
        source,
    })?;
    if is_new {
        conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
    }
    apply_pragmas(&conn)?;
    Ok(conn)
}

/// Opens the small read-only connection pool. Safe under WAL mode: readers
/// never block the single writer, and vice versa.
pub fn open_read_pool(path: &Path) -> Result<ReadPool, RepositoryError> {
    let manager = SqliteConnectionManager::file(path)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)
        .with_init(|conn| apply_read_pragmas(conn));
    r2d2::Pool::builder()
        .max_size(READ_POOL_SIZE)
        .build(manager)
        .map_err(RepositoryError::from)
}

fn apply_read_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(())
}
