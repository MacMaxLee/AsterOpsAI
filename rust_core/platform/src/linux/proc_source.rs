//! Abstracts a single `/proc`/`/sys` text read so every Linux telemetry
//! parser in `core::telemetry` can be exercised against captured fixtures
//! instead of the live machine (TRS §6). This is the ONLY production
//! implementation permitted to call `std::fs::read`/`read_to_string` for
//! telemetry purposes — every parser goes through this trait instead.

use std::io;

/// `path` is always relative, no leading `/` (e.g. `"proc/stat"`,
/// `"proc/self/cgroup"`, `"sys/fs/cgroup/user.slice/memory.max"`).
///
/// A path ending in `/.listing` is not a real kernel file — it's how a
/// directory *enumeration* (there is no single procfs file that lists every
/// PID) is expressed uniformly through this one-method trait.
/// [`RealProcSource`] answers it with a real `read_dir`; a fixture-backed
/// test implementation just reads a literal file containing newline-
/// separated names.
pub trait ProcSource: Send + Sync {
    fn read(&self, path: &str) -> io::Result<String>;
}

pub struct RealProcSource;

impl ProcSource for RealProcSource {
    fn read(&self, path: &str) -> io::Result<String> {
        if let Some(dir) = path.strip_suffix("/.listing") {
            let mut names: Vec<String> = std::fs::read_dir(format!("/{dir}"))?
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            return Ok(names.join("\n"));
        }
        std::fs::read_to_string(format!("/{path}"))
    }
}
