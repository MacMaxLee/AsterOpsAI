//! System status and uptime telemetry for macOS.
//!
//! Uses `sysctl kern.boottime` to retrieve system boot time and calculate uptime.
//! Unlike Linux's `/proc/uptime`, macOS requires FFI syscall to access boot time.

use std::collections::BTreeMap;
use std::mem::MaybeUninit;

use contracts::telemetry::{CpuPressure, MemoryPressure, SystemStatusResponse};
use contracts::{Capability, CapabilityFamily};

use super::context::SampleContext;
use super::error::TelemetryError;

/// Represents the C struct timeval from sys/time.h
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct Timeval {
    tv_sec: libc::time_t,
    tv_usec: libc::suseconds_t,
}

/// Gets the system boot time via sysctl kern.boottime.
///
/// # SAFETY
/// sysctlbyname is called with:
/// - A null-terminated string "kern.boottime\0"
/// - A properly-sized buffer for struct timeval
/// - oldlenp pointing to valid size_t
/// The function checks return code before assuming buffer is initialized.
#[allow(unsafe_code)]
fn get_boot_time() -> Result<i64, TelemetryError> {
    let name = b"kern.boottime\0";
    let mut boottime = MaybeUninit::<Timeval>::zeroed();
    let mut len = std::mem::size_of::<Timeval>();

    // SAFETY: sysctlbyname is called with valid parameters:
    // - name is a null-terminated C string
    // - boottime.as_mut_ptr() points to properly aligned Timeval storage
    // - len is the correct size for Timeval
    // Return value is checked before assuming boottime is initialized.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            boottime.as_mut_ptr() as *mut libc::c_void,
            &mut len as *mut usize,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 {
        return Err(TelemetryError::Io {
            path: "sysctl kern.boottime".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    // SAFETY: rc == 0, so sysctlbyname succeeded and boottime is initialized
    let boottime = unsafe { boottime.assume_init() };

    Ok(boottime.tv_sec)
}

/// Builds SystemStatusResponse with uptime and system-wide metrics.
///
/// Unlike Linux version, does not take a ProcSource parameter since macOS
/// uses system APIs instead of /proc filesystem.
#[allow(clippy::too_many_arguments)]
pub fn build_system_status_response(
    ctx: &SampleContext,
    containerized: bool,
    cpu_pressure: CpuPressure,
    memory_pressure: MemoryPressure,
    sample_interval_ms: u64,
    capabilities: BTreeMap<CapabilityFamily, Capability>,
) -> Result<SystemStatusResponse, TelemetryError> {
    let boot_time = get_boot_time()?;
    let uptime_seconds = (ctx.now.timestamp() - boot_time) as u64;

    Ok(SystemStatusResponse {
        timestamp: ctx.now,
        uptime_seconds,
        containerized,
        cpu_pressure,
        memory_pressure,
        sample_interval_ms,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_is_positive() {
        let ctx = SampleContext {
            now: chrono::Utc::now(),
            elapsed: std::time::Duration::from_secs(1),
            configured_interval: std::time::Duration::from_secs(1),
        };

        let response = build_system_status_response(
            &ctx,
            false,
            CpuPressure::Normal,
            MemoryPressure::Normal,
            1000,
            BTreeMap::new(),
        )
        .expect("system status should succeed");

        assert!(
            response.uptime_seconds > 0,
            "uptime should be > 0, got {}",
            response.uptime_seconds
        );

        // Sanity check: uptime shouldn't be more than 1 year (365 days)
        let one_year_seconds = 365 * 24 * 60 * 60;
        assert!(
            response.uptime_seconds < one_year_seconds,
            "uptime suspiciously high: {} seconds",
            response.uptime_seconds
        );
    }

    #[test]
    fn timestamp_matches_context() {
        let now = chrono::Utc::now();
        let ctx = SampleContext {
            now,
            elapsed: std::time::Duration::from_secs(1),
            configured_interval: std::time::Duration::from_secs(1),
        };

        let response = build_system_status_response(
            &ctx,
            false,
            CpuPressure::Normal,
            MemoryPressure::Normal,
            1000,
            BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(response.timestamp, now);
    }

    #[test]
    fn preserves_pressure_and_capabilities() {
        let ctx = SampleContext {
            now: chrono::Utc::now(),
            elapsed: std::time::Duration::from_secs(1),
            configured_interval: std::time::Duration::from_secs(1),
        };

        let mut caps = BTreeMap::new();
        caps.insert(
            CapabilityFamily::Cpu,
            Capability::Supported,
        );

        let response = build_system_status_response(
            &ctx,
            true, // containerized
            CpuPressure::High,
            MemoryPressure::Critical,
            5000,
            caps.clone(),
        )
        .unwrap();

        assert!(response.containerized);
        assert_eq!(response.cpu_pressure, CpuPressure::High);
        assert_eq!(response.memory_pressure, MemoryPressure::Critical);
        assert_eq!(response.sample_interval_ms, 5000);
        assert_eq!(response.capabilities, caps);
    }
}
