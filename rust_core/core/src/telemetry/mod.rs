//! Real Linux telemetry (unit U1). Every parser here takes `&dyn
//! platform::linux::ProcSource` and never touches `std::fs` directly, so it
//! can be exercised against the fixtures in `/tests/fixtures/proc/` (see
//! `testing::FixtureProcSource` below) as well as the live machine.

pub mod context;
pub mod cpu;
pub mod device;
pub mod error;
pub mod memory;
pub mod network;
pub mod pressure;
pub mod process;
pub mod process_classify;
pub(crate) mod rate;
pub mod storage;
pub mod system_status;

pub use context::SampleContext;
pub use cpu::{parse_cpu_snapshot, PrevCpuState};
pub use device::{parse_devices, DeviceCandidate};
pub use error::TelemetryError;
pub use memory::parse_memory_snapshot;
pub use network::{parse_network_snapshot, PrevNetState};
pub use process::{parse_process_snapshot, PrevProcessState};
pub use process_classify::{classify_process, ProcessClassifyInput};
pub use storage::{parse_storage_snapshot, PrevDiskState};
pub use system_status::build_system_status_response;

use platform::linux::ProcSource;

/// Reads this process's own cgroup v2 path from `proc/self/cgroup`
/// (`"0::/relative/path"` on the unified hierarchy) — shared by every parser
/// that needs to resolve `sys/fs/cgroup/<own-path>/...`. Returns `None` when
/// the line is absent/malformed or this isn't a cgroup v2 unified mount.
pub(crate) fn cgroup_own_path(source: &dyn ProcSource) -> Option<String> {
    let raw = source.read("proc/self/cgroup").ok()?;
    parse_cgroup_v2_path(&raw)
}

/// Extracts the unified-hierarchy path from a `/proc/[pid]/cgroup`-shaped
/// string (`"0::/relative/path"`).
pub(crate) fn parse_cgroup_v2_path(raw: &str) -> Option<String> {
    for line in raw.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            return Some(path.to_string());
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod testing {
    use platform::linux::ProcSource;
    use std::path::PathBuf;

    /// Reads from `tests/fixtures/proc/<scenario>[/phase]` at the workspace
    /// root (not any crate-local `tests/` dir — see the fixture SCOPE note
    /// in the U1 plan).
    pub struct FixtureProcSource {
        root: PathBuf,
    }

    impl FixtureProcSource {
        pub fn scenario(name: &str) -> Self {
            Self {
                root: workspace_fixture_dir(name),
            }
        }

        pub fn scenario_phase(name: &str, phase: &str) -> Self {
            Self {
                root: workspace_fixture_dir(name).join(phase),
            }
        }
    }

    fn workspace_fixture_dir(scenario: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/proc")
            .join(scenario)
    }

    impl ProcSource for FixtureProcSource {
        fn read(&self, path: &str) -> std::io::Result<String> {
            let full = self.root.join(path);
            match std::fs::read_to_string(&full) {
                Ok(contents) => Ok(contents),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    let marker = self.root.join(format!("{path}.permission_denied"));
                    if marker.exists() {
                        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                    } else {
                        Err(err)
                    }
                }
                Err(err) => Err(err),
            }
        }
    }
}
