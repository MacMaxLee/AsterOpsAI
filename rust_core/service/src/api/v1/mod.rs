pub mod cpu;
pub mod devices;
pub mod health;
pub mod history;
pub mod memory;
pub mod network;
pub mod processes;
pub mod storage;
pub mod system_status;

use axum::routing::get;
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
}
