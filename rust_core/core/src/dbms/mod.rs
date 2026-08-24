//! Read-only PostgreSQL monitoring (unit U4, SRS §26-§34, TRS §17/§33).
//!
//! A real, independently callable, integration-tested `DbmsAdapter`
//! implementation plus connection pooling, credential storage, and
//! persistence schema. U4's own doc comment once described this as
//! collection/storage only, "not yet wired into `service`'s HTTP
//! surface, a background poll loop, or the console" — every later unit
//! this named as future work has since shipped: `service::api::v1::
//! dbms` wires the full read surface over HTTP, `service::
//! dbms_security_sweep` gives its security/config checks (unit U72,
//! docs/adr/0078) a real independent poll loop, and
//! `console/lib/screens/database_screen.dart` is the console surface.
//! The wire types this surface actually needs now live in
//! `contracts::dbms`, not just internally here.

pub mod adapters;
pub mod capability;
pub mod connection_metadata;
pub mod credential_store;
pub mod log_tail;
pub mod pool;
pub mod privacy;
pub mod role_check;
pub mod version_matrix;

use std::path::PathBuf;

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
    /// `pg_stat_database.xact_commit`/`.xact_rollback` — lifetime cumulative
    /// counters (reset only on server restart or `pg_stat_reset()`), added
    /// in unit U5 as the only data source for rollback-ratio classification
    /// (SRS FR-PERF-003). Not a rate for the same reason `TempFileActivity`/
    /// `DeadlockInfo` aren't: no live poll loop persists history to diff
    /// against yet.
    pub xact_commit: i64,
    pub xact_rollback: i64,
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

/// Unit U56 (SRS FR-DBSEC-001(b)): detection input only, never a
/// `contracts` wire type — no HTTP endpoint exposes raw role data.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleSuperuserFlag {
    pub rolname: String,
    pub rolsuper: bool,
}

/// Unit U58 (SRS FR-DBSEC-001(b)'s deferred remainder): a real
/// `pg_auth_members` row — `member` is a member of `granted_role`.
/// Detection input only, same as `RoleSuperuserFlag`.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleMembership {
    pub member: String,
    pub granted_role: String,
}

/// Unit U59 (SRS FR-DBSEC-001(c), deliberately narrowed — see
/// docs/adr/0064): a real, non-owner, non-system-schema table
/// privilege grant. Detection input only, same as `RoleMembership`.
#[derive(Debug, Clone, PartialEq)]
pub struct TablePrivilegeGrant {
    pub grantee: String,
    pub schema: String,
    pub table: String,
    pub privilege_type: String,
}

/// Unit U60 (SRS FR-DBSEC-001(a)): the pure-SQL half of auth-failure
/// detection — where the PostgreSQL server writes its own log, and
/// whether it's writing it in the structured `csvlog` format `core::
/// dbms::log_tail` can parse. `log_dir` is always absolute (joined
/// against `data_directory` when PostgreSQL's own `log_directory`
/// setting is relative, matching PostgreSQL's own documented default
/// behavior). Detection input only, same as `RoleSuperuserFlag`.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthFailureLogConfig {
    pub csv_logging_enabled: bool,
    pub log_dir: PathBuf,
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
    /// Unit U56 (SRS FR-DBSEC-001(b)): every role's `rolsuper` flag —
    /// detection input for `security::detect_role_superuser_granted`,
    /// not a raw HTTP-exposed listing.
    async fn role_superuser_flags(&self) -> Result<Vec<RoleSuperuserFlag>, DbmsError>;
    /// Unit U58 (SRS FR-DBSEC-001(b)'s deferred remainder): every real
    /// `pg_auth_members` row — detection input for `security::
    /// detect_role_membership_granted`, not a raw HTTP-exposed listing.
    async fn role_memberships(&self) -> Result<Vec<RoleMembership>, DbmsError>;
    /// Unit U59 (SRS FR-DBSEC-001(c), deliberately narrowed): every
    /// real, non-owner, non-system-schema table privilege grant —
    /// detection input for `security::detect_table_privilege_granted`,
    /// not a raw HTTP-exposed listing.
    async fn table_privilege_grants(&self) -> Result<Vec<TablePrivilegeGrant>, DbmsError>;
    /// Unit U60 (SRS FR-DBSEC-001(a)): where (and whether) this
    /// instance's own auth-failure-carrying server log lives — pure
    /// SQL, unlike `log_tail`'s actual file reading, which is
    /// deliberately NOT a trait method (see that module's own doc
    /// comment for why).
    async fn auth_failure_log_config(&self) -> Result<AuthFailureLogConfig, DbmsError>;
}
