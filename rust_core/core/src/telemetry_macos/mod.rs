//! macOS telemetry implementation using Mach APIs.
//!
//! Mirrors `telemetry/` module structure but uses macOS-specific APIs:
//! - `host_statistics64()` for CPU ticks
//! - `vm_statistics64()` for memory stats
//! - `statfs()` for storage capacity
//! - `libproc` for process enumeration
//!
//! Established in unit U95 (CPU telemetry); expanded in U96-U100 for other
//! telemetry sources.

pub mod context;
pub mod cpu;
pub mod error;
pub mod memory;
mod rate;

pub use context::SampleContext;
pub use cpu::{parse_cpu_snapshot, PrevCpuState};
pub use error::TelemetryError;
pub use memory::parse_memory_snapshot;
