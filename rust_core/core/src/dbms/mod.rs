//! Read-only PostgreSQL monitoring (unit U4, SRS §26-§34, TRS §17/§33).
//!
//! **Scope note**: this module is the collection/storage layer only — a
//! real, independently callable, integration-tested `DbmsAdapter`
//! implementation plus connection pooling, credential storage, and
//! persistence schema. It is not yet wired into `service`'s HTTP surface,
//! a background poll loop, or the console (see the U4 plan's SCOPE
//! amendment note); that wiring is later-unit work, matching the same
//! "flag the amendment, don't silently expand scope" precedent U2 and U3
//! used for their own SCOPE-vs-REQUIREMENTS gaps.
//!
//! These are internal Rust types, not wire contracts — there is no API
//! layer consuming them yet, so they don't belong in `contracts` (which
//! `emit_schemas.rs` would otherwise need to cover for nothing).

pub mod adapters;
pub mod capability;
pub mod connection_metadata;
pub mod credential_store;
pub mod pool;
pub mod privacy;
pub mod role_check;
pub mod version_matrix;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub use capability::Gated;
pub use connection_metadata::{ConnectionMetadata, Environment, PasswordRef, TlsMode};
pub use credential_store::{CredentialStore, CredentialStoreError, KeyringCredentialStore};

#[derive(Debug, thiserror::Error)]
pub enum DbmsError {
    #[error("database error: {0}")]
    Query(#[from] tokio_postgres::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("connection refused: {0}")]
    PermissionDenied(String),

    #[error("credential store error: {0}")]
    Credential(#[from] CredentialStoreError),

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionInfo {
    pub version_string: String,
    /// `SHOW server_version_num`'s integer encoding, e.g. `170002` for
    /// 17.2 — the same encoding `version_matrix.rs` compares against.
    pub server_version_num: i32,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseInfo {
    pub name: String,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Idle,
    IdleInTransaction,
    /// Derived from `wait_event_type IS NOT NULL`, not from `state` alone —
    /// a session can be `active` and waiting at the same time.
    Waiting,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub pid: i32,
    pub username: Option<String>,
    pub database: Option<String>,
    pub state: SessionState,
    pub client_addr: Option<String>,
    pub xact_start: Option<DateTime<Utc>>,
    pub query_start: Option<DateTime<Utc>>,
    /// The session's current/last query text, sanitized per TRS §19 by
    /// `privacy::sanitize_query` before it ever reaches this struct — never
    /// the driver's raw string passed straight through (see that module's
    /// doc comment for why `pg_stat_activity`, not `pg_stat_statements`, is
    /// this codebase's actual raw-SQL source).
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryStat {
    /// `pg_stat_statements.queryid` — already a normalized-query hash, not
    /// a second fingerprint computed on top of it.
    pub query_fingerprint: String,
    /// `pg_stat_statements.query` is *always* pre-normalized by Postgres
    /// itself (constants replaced with `$1`-style placeholders) — there is
    /// no raw-SQL variant available from this source at all, so unlike
    /// `SessionInfo.query` there is no `capture_raw_sql`-gated raw form
    /// here to omit or include.
    pub normalized_query: String,
    pub calls: i64,
    pub total_exec_time_ms: f64,
    pub mean_exec_time_ms: f64,
    pub rows: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockEdge {
    pub blocked_pid: i32,
    pub blocked_query: Option<String>,
    pub blocking_pid: i32,
    pub blocking_query: Option<String>,
    pub lock_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableStat {
    pub schema: String,
    pub table: String,
    pub seq_scan: i64,
    pub idx_scan: i64,
    pub n_live_tup: i64,
    pub n_dead_tup: i64,
    pub last_vacuum: Option<DateTime<Utc>>,
    pub last_autovacuum: Option<DateTime<Utc>>,
    pub total_size_bytes: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexStat {
    pub schema: String,
    pub table: String,
    pub index: String,
    pub idx_scan: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StandbyInfo {
    pub client_addr: Option<String>,
    pub state: String,
    /// LSNs kept as their native `pg_lsn` text form (e.g. `"0/1A2B3C4"`) —
    /// no numeric LSN math happens in this unit, so no premature u64
    /// parsing/representation is introduced for it.
    pub sent_lsn: Option<String>,
    pub write_lsn: Option<String>,
    pub flush_lsn: Option<String>,
    pub replay_lsn: Option<String>,
    pub replay_lag_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplicationStatus {
    pub is_primary: bool,
    pub in_recovery: bool,
    pub standbys: Vec<StandbyInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GucValue {
    pub name: String,
    pub setting: String,
    pub unit: Option<String>,
    pub source: String,
}

/// Raw cumulative counters as PostgreSQL reports them (reset only on
/// server restart or an explicit `pg_stat_reset()`), plus `stats_reset` so
/// a caller can tell whether a lower reading means "less activity" or "the
/// counter reset." Deliberately not a computed rate: rate calculation needs
/// a remembered previous sample, which belongs to whichever later unit
/// wires up a live poll loop (SCOPE note above) — this stays a stateless,
/// honest snapshot, matching `platform::PlatformAdapter`'s own precedent of
/// never computing rates itself.
#[derive(Debug, Clone, PartialEq)]
pub struct TempFileActivity {
    pub temp_files: i64,
    pub temp_bytes: i64,
    pub stats_reset: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeadlockInfo {
    pub deadlocks: i64,
    pub stats_reset: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LongTransaction {
    pub pid: i32,
    pub username: Option<String>,
    pub duration_seconds: f64,
    pub state: SessionState,
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdleInTransactionSession {
    pub pid: i32,
    pub username: Option<String>,
    pub idle_duration_seconds: f64,
}

/// TRS §33's exact method set. Read-only through U4 — the action surface
/// (`analyze_table`, etc.) arrives in U8 under policy gating, never a
/// generic "execute SQL" escape hatch (FORBIDDEN, this unit and every
/// unit after it).
#[async_trait]
pub trait DbmsAdapter: Send + Sync {
    async fn version_info(&self) -> Result<VersionInfo, DbmsError>;
    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>, DbmsError>;
    async fn list_sessions(&self) -> Result<Vec<SessionInfo>, DbmsError>;
    async fn query_stats(&self) -> Result<Gated<Vec<QueryStat>>, DbmsError>;
    async fn lock_graph(&self) -> Result<Vec<LockEdge>, DbmsError>;
    async fn table_stats(&self) -> Result<Vec<TableStat>, DbmsError>;
    async fn index_stats(&self) -> Result<Vec<IndexStat>, DbmsError>;
    async fn replication_status(&self) -> Result<ReplicationStatus, DbmsError>;
    async fn relevant_gucs(&self) -> Result<Vec<GucValue>, DbmsError>;
    async fn temp_file_activity(&self) -> Result<TempFileActivity, DbmsError>;
    async fn deadlock_history(&self) -> Result<DeadlockInfo, DbmsError>;
    async fn long_transactions(&self) -> Result<Vec<LongTransaction>, DbmsError>;
    async fn idle_in_transaction_sessions(
        &self,
    ) -> Result<Vec<IdleInTransactionSession>, DbmsError>;
}
