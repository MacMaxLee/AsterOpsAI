// `pub(crate)`, not private: `role_check.rs` (outside this adapters/
// module) reaches `current_user_is_superuser` through here rather than
// embedding its own copy of that query — every PostgreSQL SQL string
// literal lives under `adapters/` (requirement 1 / the grep gate), no
// exceptions for a query that happens to be used from elsewhere too.
pub(crate) mod queries;

use async_trait::async_trait;
use deadpool_postgres::Pool;

use crate::dbms::{
    capability::Gated, connection_metadata::ConnectionMetadata, credential_store::CredentialStore,
    pool, role_check, DatabaseInfo, DbmsAdapter, DbmsError, DeadlockInfo, GucValue,
    IdleInTransactionSession, IndexStat, LockEdge, LongTransaction, QueryStat, ReplicationStatus,
    SessionInfo, TableStat, TempFileActivity, VersionInfo,
};

/// A row is "long-running" past this many seconds of open-transaction time
/// (`long_transactions`) or idle-in-transaction time
/// (`idle_in_transaction_sessions`) — not user-configurable this unit
/// (SCOPE doesn't wire up a live config surface yet; see mod.rs's scope
/// note), but named as a constant rather than a buried literal so a later
/// unit can make it configurable without hunting for it.
const LONG_TRANSACTION_THRESHOLD_SECONDS: f64 = 60.0;
const IDLE_IN_TRANSACTION_THRESHOLD_SECONDS: f64 = 60.0;

pub struct PostgresAdapter {
    pool: Pool,
    capture_raw_sql: bool,
}

impl PostgresAdapter {
    /// Fetches the password from `credential_store`, builds the pool
    /// (requirement 2's connection settings), and runs the least-privilege
    /// check (requirement 4) against a real connection before returning —
    /// a `PostgresAdapter` that exists at all has already passed that
    /// check once.
    pub async fn connect(
        metadata: &ConnectionMetadata,
        credential_store: &dyn CredentialStore,
    ) -> Result<Self, DbmsError> {
        let password = credential_store.fetch(&metadata.password_ref)?;
        let pool = pool::build_pool(metadata, &password)?;

        let client = pool.get().await?;
        role_check::validate(
            &client,
            metadata.environment,
            metadata.allow_superuser_override,
        )
        .await?;
        drop(client);

        Ok(Self {
            pool,
            capture_raw_sql: metadata.capture_raw_sql,
        })
    }

    /// An alternate constructor for a caller that already has a configured
    /// `Pool` (e.g. integration tests pointed at a fixture instance, or a
    /// future caller reusing a pool across adapters) and doesn't need to
    /// round-trip through the credential store — `connect` above remains
    /// the only entry point that performs the least-privilege check.
    pub fn from_pool(pool: Pool, capture_raw_sql: bool) -> Self {
        Self {
            pool,
            capture_raw_sql,
        }
    }
}

// `self.pool.get().await?` is bound to a local before every query call
// (rather than inlined as `&self.pool.get().await?`) deliberately: rustc's
// type inference for the `?` operator gets confused when the borrow and
// the multi-hop `Object<Manager> -> ClientWrapper -> tokio_postgres::
// Client` deref coercion are both resolved at the same expression site,
// misreporting a `?`-conversion error between `Object<Manager>` and
// `Client` — a separate `let` statement resolves the ambiguity.
#[async_trait]
impl DbmsAdapter for PostgresAdapter {
    async fn version_info(&self) -> Result<VersionInfo, DbmsError> {
        let client = self.pool.get().await?;
        queries::version_info(&client).await
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>, DbmsError> {
        let client = self.pool.get().await?;
        queries::list_databases(&client).await
    }

    async fn list_sessions(&self) -> Result<Vec<SessionInfo>, DbmsError> {
        let client = self.pool.get().await?;
        queries::list_sessions(&client, self.capture_raw_sql).await
    }

    async fn query_stats(&self) -> Result<Gated<Vec<QueryStat>>, DbmsError> {
        let client = self.pool.get().await?;
        queries::query_stats(&client).await
    }

    async fn lock_graph(&self) -> Result<Vec<LockEdge>, DbmsError> {
        let client = self.pool.get().await?;
        queries::lock_graph(&client).await
    }

    async fn table_stats(&self) -> Result<Vec<TableStat>, DbmsError> {
        let client = self.pool.get().await?;
        queries::table_stats(&client).await
    }

    async fn index_stats(&self) -> Result<Vec<IndexStat>, DbmsError> {
        let client = self.pool.get().await?;
        queries::index_stats(&client).await
    }

    async fn replication_status(&self) -> Result<ReplicationStatus, DbmsError> {
        let client = self.pool.get().await?;
        queries::replication_status(&client).await
    }

    async fn relevant_gucs(&self) -> Result<Vec<GucValue>, DbmsError> {
        let client = self.pool.get().await?;
        queries::relevant_gucs(&client).await
    }

    async fn temp_file_activity(&self) -> Result<TempFileActivity, DbmsError> {
        let client = self.pool.get().await?;
        queries::temp_file_activity(&client).await
    }

    async fn deadlock_history(&self) -> Result<DeadlockInfo, DbmsError> {
        let client = self.pool.get().await?;
        queries::deadlock_history(&client).await
    }

    async fn long_transactions(&self) -> Result<Vec<LongTransaction>, DbmsError> {
        let client = self.pool.get().await?;
        queries::long_transactions(
            &client,
            LONG_TRANSACTION_THRESHOLD_SECONDS,
            self.capture_raw_sql,
        )
        .await
    }

    async fn idle_in_transaction_sessions(
        &self,
    ) -> Result<Vec<IdleInTransactionSession>, DbmsError> {
        let client = self.pool.get().await?;
        queries::idle_in_transaction_sessions(&client, IDLE_IN_TRANSACTION_THRESHOLD_SECONDS).await
    }
}
