//! Library half of the AsterOpsAI core service, split from `main.rs` so
//! integration tests can exercise the router directly without a real socket.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod api;
pub mod config;
pub mod middleware;
pub mod response;
pub mod retention;
pub mod self_metrics;
pub mod state;
pub mod telemetry;
pub mod transport;

pub use state::AppState;
