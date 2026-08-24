//! Telemetry error types for macOS.
//!
//! Mirrors `telemetry/error.rs` but for macOS-specific telemetry operations.
//! Reserved for genuinely fatal parse failures on data the parser cannot do
//! without. Everything soft (permission denied, unavailable metrics, etc.) is
//! represented inline as `MetricValue::Unavailable`, never propagated as `Err`.

/// Errors that can occur during macOS telemetry data collection.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// I/O error reading from a Mach API or system source.
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Parse error when processing macOS system data.
    #[error("failed to parse {path}: {reason}")]
    Parse { path: String, reason: String },
}
