/// Reserved for genuinely fatal parse failures on a file a parser cannot do
/// without (e.g. `/proc/stat` itself missing or malformed). Everything
/// "soft" — a permission-denied per-process file, an absent cpufreq node —
/// is represented inline as `MetricValue::Unavailable`/
/// `Capability::PermissionRequired`, never propagated as an `Err` here.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {reason}")]
    Parse { path: String, reason: String },
}
