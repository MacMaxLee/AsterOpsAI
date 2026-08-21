pub mod actions;
pub mod analysis;
pub mod cpu;
pub mod dbms;
pub mod devices;
pub mod health;
pub mod history;
pub mod memory;
pub mod network;
pub mod policy;
pub mod processes;
pub mod security;
pub mod storage;
pub mod system_status;
pub mod tuning;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/cpu", get(cpu::cpu))
        .route("/memory", get(memory::memory))
        .route("/storage", get(storage::storage))
        .route("/network", get(network::network))
        .route("/processes", get(processes::processes))
        .route("/devices", get(devices::devices))
        .route("/system/status", get(system_status::system_status))
        .route("/history/cpu", get(history::cpu_history))
        .route("/history/memory", get(history::memory_history))
        .route("/history/storage", get(history::storage_history))
        .route("/history/network", get(history::network_history))
        .route("/actions/propose", post(actions::propose))
        .route("/actions/resumable", get(actions::resumable))
        .route("/policy/pending", get(policy::pending))
        .route("/policy/:id/grant", post(policy::grant))
        .route("/policy/:id/reject", post(policy::reject))
        .route("/policy/:id/rollback", post(policy::rollback))
        .route("/tuning/plans", get(tuning::plans))
        .route("/tuning/start", post(tuning::start))
        .route("/security/incidents", get(security::incidents))
        .route("/security/suppress", post(security::suppress))
        .route("/analysis/host", get(analysis::host))
        .route("/analysis/host/explain", get(analysis::explain_host))
        .route("/analysis/correlation", get(analysis::correlation))
        .route("/dbms/sessions", get(dbms::sessions))
        .route("/dbms/locks", get(dbms::locks))
        .route("/dbms/query-stats", get(dbms::query_stats))
        .route("/dbms/table-stats", get(dbms::table_stats))
        .route("/dbms/index-stats", get(dbms::index_stats))
        .route("/dbms/replication", get(dbms::replication))
        .route("/dbms/gucs", get(dbms::gucs))
        .route("/dbms/temp-file-activity", get(dbms::temp_file_activity))
        .route("/dbms/deadlock-history", get(dbms::deadlock_history))
        .route("/dbms/long-transactions", get(dbms::long_transactions))
        .route(
            "/dbms/idle-in-transaction-sessions",
            get(dbms::idle_in_transaction_sessions),
        )
}
