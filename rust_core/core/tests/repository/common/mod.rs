//! Shared helpers for the repository integration tests. Integration tests
//! are already test-only code; the workspace's unwrap/expect deny targets
//! production code paths, not test files.
#![allow(dead_code)]

use std::path::PathBuf;

use ai_ops_core::repository::{self, RepositoryConfig, RepositoryHandle};
use tempfile::TempDir;

/// A temp-file-backed repository, kept alive for the test's duration (the
/// `TempDir` is dropped, deleting the file, only when this struct is).
pub struct TestRepo {
    pub handle: RepositoryHandle,
    db_path: PathBuf,
    _dir: TempDir,
}

impl TestRepo {
    pub fn open() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("test.sqlite3");
        let handle = repository::init(&RepositoryConfig {
            db_path: db_path.clone(),
        })
        .expect("repository::init");
        Self {
            handle,
            db_path,
            _dir: dir,
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}
