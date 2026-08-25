//! macOS Memory telemetry via Mach `host_statistics64` and `sysctl`.
//!
//! Collects system-wide memory statistics using:
//! - `sysctl hw.memsize` for total physical RAM
//! - `host_statistics64(HOST_VM_INFO64)` for VM statistics
//! - `sysctl vm.swapusage` for swap statistics
//!
//! Mirrors `telemetry/memory.rs` structure but uses macOS-specific APIs.
//! Established in unit U96.

use contracts::telemetry::{MemoryPressure, MemorySnapshot, MetricValue};

use super::context::SampleContext;
use super::error::TelemetryError;

// ============================================================================
// FFI Bindings for Mach APIs and sysctl
// ============================================================================

const HOST_VM_INFO64: i32 = 4;
const HOST_VM_INFO64_COUNT: u32 = 38;
const KERN_SUCCESS: i32 = 0;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[allow(non_camel_case_types)]
struct vm_statistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
struct xsw_usage {
    xsu_total: u64,
    xsu_avail: u64,
    xsu_used: u64,
    xsu_pagesize: u32,
    xsu_encrypted: bool,
}

#[link(name = "c")]
extern "C" {
    fn mach_host_self() -> u32;

    fn host_statistics64(
        host_priv: u32,
        flavor: i32,
        host_info_out: *mut i32,
        host_info_outCnt: *mut u32,
    ) -> i32;

    fn sysctlbyname(
        name: *const libc::c_char,
        oldp: *mut libc::c_void,
        oldlenp: *mut libc::size_t,
        newp: *mut libc::c_void,
        newlen: libc::size_t,
    ) -> libc::c_int;
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Gets total physical memory via sysctl hw.memsize.
#[allow(unsafe_code)]
fn get_total_physical_memory() -> Result<u64, TelemetryError> {
    let mut size: u64 = 0;
    let mut len = std::mem::size_of::<u64>();

    let name = b"hw.memsize\0";

    // SAFETY: `sysctlbyname` reads the NUL-terminated name string and writes
    // to the `size` output buffer. We check the return code before using the
    // value.
    let rc = unsafe {
        sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            &mut size as *mut u64 as *mut libc::c_void,
            &mut len as *mut usize,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 {
        return Err(TelemetryError::Io {
            path: "sysctl(hw.memsize)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    Ok(size)
}

/// Gets VM page size via sysctl hw.pagesize.
#[allow(unsafe_code)]
fn get_page_size() -> Result<u64, TelemetryError> {
    let mut pagesize: i32 = 0;
    let mut len = std::mem::size_of::<i32>();

    let name = b"hw.pagesize\0";

    // SAFETY: `sysctlbyname` reads the NUL-terminated name string and writes
    // to the `pagesize` output buffer. We check the return code before using
    // the value.
    let rc = unsafe {
        sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            &mut pagesize as *mut i32 as *mut libc::c_void,
            &mut len as *mut usize,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 {
        return Err(TelemetryError::Io {
            path: "sysctl(hw.pagesize)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    Ok(pagesize as u64)
}

/// Gets VM statistics via host_statistics64(HOST_VM_INFO64).
///
/// # SAFETY (documented at call site below)
/// host_statistics64 operates on a Mach port and a fixed-size output buffer.
#[allow(unsafe_code)]
fn get_vm_statistics() -> Result<vm_statistics64, TelemetryError> {
    let mut stats = std::mem::MaybeUninit::<vm_statistics64>::zeroed();
    let mut count = HOST_VM_INFO64_COUNT;

    // SAFETY: `stats` is a valid, correctly-sized writable buffer;
    // `host_statistics64` only ever writes to it and returns a status code
    // we check before treating it as initialized. `mach_host_self()` returns
    // a well-known Mach port that doesn't need deallocation.
    let rc = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            stats.as_mut_ptr() as *mut i32,
            &mut count,
        )
    };

    if rc != KERN_SUCCESS {
        return Err(TelemetryError::Io {
            path: "host_statistics64(HOST_VM_INFO64)".to_string(),
            source: std::io::Error::from_raw_os_error(rc),
        });
    }

    // SAFETY: `rc == KERN_SUCCESS` guarantees the kernel fully populated `stats`.
    Ok(unsafe { stats.assume_init() })
}

/// Gets swap usage via sysctl vm.swapusage.
#[allow(unsafe_code)]
fn get_swap_usage() -> Result<xsw_usage, TelemetryError> {
    let mut swap = std::mem::MaybeUninit::<xsw_usage>::zeroed();
    let mut len = std::mem::size_of::<xsw_usage>();

    let name = b"vm.swapusage\0";

    // SAFETY: `sysctlbyname` reads the NUL-terminated name string and writes
    // to the `swap` output buffer. We check the return code before using the
    // value.
    let rc = unsafe {
        sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            swap.as_mut_ptr() as *mut libc::c_void,
            &mut len as *mut usize,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 {
        return Err(TelemetryError::Io {
            path: "sysctl(vm.swapusage)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    // SAFETY: `rc == 0` guarantees sysctlbyname fully populated `swap`.
    Ok(unsafe { swap.assume_init() })
}

/// Classifies memory pressure based on available/total ratio and swap usage.
/// Mirrors `telemetry/pressure.rs::classify_memory_pressure` logic.
fn classify_memory_pressure(
    total_bytes: u64,
    available_bytes: u64,
    swap_total_bytes: u64,
    swap_used_bytes: u64,
) -> MemoryPressure {
    // Delegate to the shared pressure classification logic if it exists,
    // or inline the thresholds here following ADR 0006.
    let available_ratio = if total_bytes == 0 {
        1.0
    } else {
        available_bytes as f64 / total_bytes as f64
    };

    // Primary signal: available/total
    let mem_tier = if available_ratio <= 0.05 {
        MemoryPressure::Critical
    } else if available_ratio <= 0.15 {
        MemoryPressure::High
    } else if available_ratio <= 0.30 {
        MemoryPressure::Elevated
    } else {
        MemoryPressure::Normal
    };

    // Secondary signal: swap usage can only raise the tier, never lower it
    if swap_total_bytes == 0 {
        return mem_tier;
    }

    let swap_ratio = swap_used_bytes as f64 / swap_total_bytes as f64;
    let swap_tier = if swap_ratio >= 0.85 {
        MemoryPressure::Critical
    } else if swap_ratio >= 0.50 {
        MemoryPressure::High
    } else if swap_ratio >= 0.25 {
        MemoryPressure::Elevated
    } else {
        MemoryPressure::Normal
    };

    // Return max(mem_tier, swap_tier)
    match (mem_tier, swap_tier) {
        (MemoryPressure::Critical, _) | (_, MemoryPressure::Critical) => MemoryPressure::Critical,
        (MemoryPressure::High, _) | (_, MemoryPressure::High) => MemoryPressure::High,
        (MemoryPressure::Elevated, _) | (_, MemoryPressure::Elevated) => MemoryPressure::Elevated,
        _ => MemoryPressure::Normal,
    }
}

// ============================================================================
// Main Parser Function
// ============================================================================

/// Parses a memory snapshot on macOS using Mach APIs and sysctl.
///
/// Returns `MemorySnapshot` with all required fields populated.
pub fn parse_memory_snapshot(
    ctx: &SampleContext,
) -> Result<MemorySnapshot, TelemetryError> {
    // Get total physical memory
    let total_bytes = get_total_physical_memory()?;
    let page_size = get_page_size()?;
    let vm_stats = get_vm_statistics()?;

    // Calculate available memory: free + inactive + speculative
    // This mirrors how macOS Activity Monitor calculates "available"
    let free_bytes = vm_stats.free_count as u64 * page_size;
    let inactive_bytes = vm_stats.inactive_count as u64 * page_size;
    let speculative_bytes = vm_stats.speculative_count as u64 * page_size;
    let available_bytes = free_bytes + inactive_bytes + speculative_bytes;

    // Calculate used memory: total - available
    let used_bytes = total_bytes.saturating_sub(available_bytes);

    // Cache and buffers: macOS doesn't distinguish these the way Linux does
    // File-backed pages are tracked in external_page_count
    let cached_bytes = vm_stats.external_page_count as u64 * page_size;
    let _buffers_bytes = 0u64; // macOS doesn't expose buffer cache separately

    // Get swap statistics
    let (swap_total_bytes, swap_used_bytes, swap_free_bytes) = match get_swap_usage() {
        Ok(swap) => (swap.xsu_total, swap.xsu_used, swap.xsu_avail),
        Err(_) => {
            // Swap might not be configured or accessible
            (0, 0, 0)
        }
    };

    // Classify pressure
    let pressure = classify_memory_pressure(
        total_bytes,
        available_bytes,
        swap_total_bytes,
        swap_used_bytes,
    );

    // macOS doesn't use cgroups
    let containerized = false;

    // NUMA nodes: macOS doesn't expose NUMA topology via standard APIs
    let numa_nodes = MetricValue::Unavailable {
        reason: "macOS does not expose NUMA node topology".to_string(),
    };

    Ok(MemorySnapshot {
        timestamp: ctx.now,
        total_bytes: MetricValue::Supported { value: total_bytes },
        used_bytes: MetricValue::Supported { value: used_bytes },
        available_bytes: MetricValue::Supported {
            value: available_bytes,
        },
        cached_bytes: MetricValue::Supported {
            value: cached_bytes,
        },
        buffers_bytes: MetricValue::Unavailable {
            reason: "macOS does not expose buffer cache separately".to_string(),
        },
        swap_total_bytes: MetricValue::Supported {
            value: swap_total_bytes,
        },
        swap_used_bytes: MetricValue::Supported {
            value: swap_used_bytes,
        },
        swap_free_bytes: MetricValue::Supported {
            value: swap_free_bytes,
        },
        pressure,
        containerized,
        numa_nodes,
    })
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn fixed_ctx() -> SampleContext {
        SampleContext {
            now: chrono::Utc::now(),
            elapsed: Duration::from_secs(1),
            configured_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn memory_snapshot_returns_valid_data() {
        let snapshot = parse_memory_snapshot(&fixed_ctx())
            .expect("memory snapshot should succeed");

        // Total memory should be > 0
        match snapshot.total_bytes {
            MetricValue::Supported { value } => {
                assert!(value > 0, "total_bytes should be > 0, got {}", value);
            }
            other => panic!("total_bytes should be Supported, got {:?}", other),
        }

        // Available memory should be > 0
        match snapshot.available_bytes {
            MetricValue::Supported { value } => {
                assert!(value > 0, "available_bytes should be > 0, got {}", value);
            }
            other => panic!("available_bytes should be Supported, got {:?}", other),
        }

        // Used memory should be > 0
        match snapshot.used_bytes {
            MetricValue::Supported { value } => {
                assert!(value > 0, "used_bytes should be > 0, got {}", value);
            }
            other => panic!("used_bytes should be Supported, got {:?}", other),
        }
    }

    #[test]
    fn pressure_tier_is_valid() {
        let snapshot = parse_memory_snapshot(&fixed_ctx())
            .expect("memory snapshot should succeed");

        // Pressure should be one of the four valid tiers
        assert!(
            matches!(
                snapshot.pressure,
                MemoryPressure::Normal
                    | MemoryPressure::Elevated
                    | MemoryPressure::High
                    | MemoryPressure::Critical
            ),
            "pressure should be a valid tier, got {:?}",
            snapshot.pressure
        );
    }

    #[test]
    fn swap_metrics_present() {
        let snapshot = parse_memory_snapshot(&fixed_ctx())
            .expect("memory snapshot should succeed");

        // Swap metrics should be Supported (even if swap is disabled, they'll be 0)
        assert!(
            matches!(snapshot.swap_total_bytes, MetricValue::Supported { .. }),
            "swap_total_bytes should be Supported"
        );
        assert!(
            matches!(snapshot.swap_used_bytes, MetricValue::Supported { .. }),
            "swap_used_bytes should be Supported"
        );
        assert!(
            matches!(snapshot.swap_free_bytes, MetricValue::Supported { .. }),
            "swap_free_bytes should be Supported"
        );
    }

    #[test]
    fn unavailable_metrics_documented() {
        let snapshot = parse_memory_snapshot(&fixed_ctx())
            .expect("memory snapshot should succeed");

        // Buffers should be unavailable on macOS
        assert!(
            matches!(snapshot.buffers_bytes, MetricValue::Unavailable { .. }),
            "buffers_bytes should be unavailable on macOS"
        );

        // NUMA nodes should be unavailable
        assert!(
            matches!(snapshot.numa_nodes, MetricValue::Unavailable { .. }),
            "numa_nodes should be unavailable on macOS"
        );
    }

    #[test]
    fn not_containerized() {
        let snapshot = parse_memory_snapshot(&fixed_ctx())
            .expect("memory snapshot should succeed");

        // macOS doesn't typically use cgroups
        assert!(!snapshot.containerized, "should not be containerized");
    }

    #[test]
    fn memory_accounting_is_consistent() {
        let snapshot = parse_memory_snapshot(&fixed_ctx())
            .expect("memory snapshot should succeed");

        // Extract values
        let total = match snapshot.total_bytes {
            MetricValue::Supported { value } => value,
            _ => panic!("total_bytes should be Supported"),
        };
        let used = match snapshot.used_bytes {
            MetricValue::Supported { value } => value,
            _ => panic!("used_bytes should be Supported"),
        };
        let available = match snapshot.available_bytes {
            MetricValue::Supported { value } => value,
            _ => panic!("available_bytes should be Supported"),
        };

        // used + available should approximately equal total
        // (allowing for some rounding or accounting differences)
        let sum = used + available;
        let diff = if sum > total {
            sum - total
        } else {
            total - sum
        };

        // Allow up to 1% difference for rounding
        let tolerance = total / 100;
        assert!(
            diff <= tolerance,
            "used ({}) + available ({}) should approximately equal total ({}), diff={}",
            used,
            available,
            total,
            diff
        );
    }
}
