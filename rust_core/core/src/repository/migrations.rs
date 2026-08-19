use rusqlite::Connection;

use super::error::RepositoryError;

// Path is relative to this crate's CARGO_MANIFEST_DIR (rust_core/core/), so
// "../../migrations" resolves to the workspace-root /migrations/ directory.
//
// A real gotcha, discovered while adding V8 (unit U4): `embed_migrations!`
// doesn't reliably make Cargo detect a *new* file appearing in the
// migrations directory — only changes to files Cargo already knew to
// track. Adding a migration and immediately `cargo test`ing against a
// still-warm incremental build can silently link a stale copy of this
// module (an already-migrated test still shows the *previous* migration
// set). Harmless for a fresh clone or CI (nothing stale to reuse), but if
// a local incremental build ever looks like it "forgot" a new migration,
// `touch` this file (or `cargo clean -p core`) to force a rebuild — that's
// a build-cache artifact, not a bug in the migration itself.
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
