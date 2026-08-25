//! macOS Storage telemetry via POSIX `statfs` and `getfsstat`.
//!
//! Collects filesystem capacity statistics using:
//! - `getfsstat()` to enumerate mounted filesystems
//! - `statfs()` to get capacity/free/available for each filesystem
//!
//! Mirrors `telemetry/storage.rs` structure but uses macOS-specific APIs.
//! Established in unit U97 (filesystem capacity); U98 will add disk I/O.

use contracts::telemetry::{MetricValue, StorageSnapshot, VolumeInfo};

use super::context::SampleContext;
use super::error::TelemetryError;

// ============================================================================
// FFI Bindings for statfs and getfsstat
// ============================================================================

// Virtual/pseudo filesystems to skip (don't represent physical storage)
const PSEUDO_FILESYSTEMS: &[&str] = &[
    "devfs",
    "autofs",
    "nullfs",
    "fdesc",
    "union",
    "kernfs",
    "procfs",
    "lofs",
    "tmpfs",
    "ctfs",
    "mntfs",
    "objfs",
    "sharefs",
];

// ============================================================================
// Helper Functions
// ============================================================================

/// Checks if a filesystem type should be excluded from telemetry.
fn is_pseudo_filesystem(fstype: &str) -> bool {
    PSEUDO_FILESYSTEMS.contains(&fstype)
}

/// Gets list of mounted filesystems via getfsstat.
///
/// # SAFETY (documented at call site below)
/// getfsstat writes to a buffer we provide, returns count of entries written.
#[allow(unsafe_code)]
fn get_mounted_filesystems() -> Result<Vec<libc::statfs>, TelemetryError> {
    // First call: get count of mounted filesystems
    // SAFETY: getfsstat with NULL buffer and size 0 just returns the count,
    // doesn't write anything.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };

    if count < 0 {
        return Err(TelemetryError::Io {
            path: "getfsstat(count)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    if count == 0 {
        return Ok(Vec::new());
    }

    // Allocate buffer for filesystem entries
    let mut buf: Vec<libc::statfs> = Vec::with_capacity(count as usize);

    // Second call: actually get the filesystem entries
    // SAFETY: `buf` is a valid, correctly-sized writable buffer for `count`
    // statfs entries. `getfsstat` writes to it and returns the number of
    // entries written, which we check before treating the buffer as
    // initialized.
    let written = unsafe {
        libc::getfsstat(
            buf.as_mut_ptr(),
            (count as usize * std::mem::size_of::<libc::statfs>()) as libc::c_int,
            libc::MNT_NOWAIT,
        )
    };

    if written < 0 {
        return Err(TelemetryError::Io {
            path: "getfsstat(read)".to_string(),
            source: std::io::Error::last_os_error(),
        });
    }

    // SAFETY: getfsstat successfully wrote `written` entries to `buf`.
    unsafe {
        buf.set_len(written as usize);
    }

    Ok(buf)
}

/// Converts a C string (null-terminated char array) to a Rust String.
fn cstr_to_string(bytes: &[libc::c_char]) -> String {
    let null_pos = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    let bytes_u8: Vec<u8> = bytes[..null_pos].iter().map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes_u8).to_string()
}

/// Converts a statfs entry to a VolumeInfo.
fn statfs_to_volume_info(stat: &libc::statfs) -> VolumeInfo {
    // Extract strings from C arrays
    let mount_point = cstr_to_string(&stat.f_mntonname);
    let device = cstr_to_string(&stat.f_mntfromname);
    let filesystem = cstr_to_string(&stat.f_fstypename);

    // Calculate capacity metrics
    let block_size = stat.f_bsize as u64;
    let capacity_bytes = stat.f_blocks as u64 * block_size;
    let free_bytes = stat.f_bfree as u64 * block_size;
    let available_bytes = stat.f_bavail as u64 * block_size;

    VolumeInfo {
        device,
        mount_point,
        filesystem,
        capacity_bytes: MetricValue::Supported {
            value: capacity_bytes,
        },
        free_bytes: MetricValue::Supported { value: free_bytes },
        available_bytes: MetricValue::Supported {
            value: available_bytes,
        },
        // Disk I/O metrics: unavailable in U97, will be added in U98
        read_bytes_per_sec: MetricValue::Unavailable {
            reason: "disk I/O metrics not yet implemented (U98)".to_string(),
        },
        write_bytes_per_sec: MetricValue::Unavailable {
            reason: "disk I/O metrics not yet implemented (U98)".to_string(),
        },
        read_ops_per_sec: MetricValue::Unavailable {
            reason: "disk I/O metrics not yet implemented (U98)".to_string(),
        },
        write_ops_per_sec: MetricValue::Unavailable {
            reason: "disk I/O metrics not yet implemented (U98)".to_string(),
        },
        io_latency_ms: MetricValue::Unavailable {
            reason: "disk I/O metrics not yet implemented (U98)".to_string(),
        },
    }
}

// ============================================================================
// Main Parser Function
// ============================================================================

/// Parses a storage snapshot on macOS using getfsstat and statfs.
///
/// Returns `StorageSnapshot` with filesystem capacity information.
/// Disk I/O metrics are marked as unavailable (will be added in U98).
pub fn parse_storage_snapshot(
    ctx: &SampleContext,
) -> Result<StorageSnapshot, TelemetryError> {
    let mounted = get_mounted_filesystems()?;

    let volumes: Vec<VolumeInfo> = mounted
        .iter()
        .filter(|stat| {
            let fstype = cstr_to_string(&stat.f_fstypename);
            !is_pseudo_filesystem(&fstype)
        })
        .map(statfs_to_volume_info)
        .collect();

    Ok(StorageSnapshot {
        timestamp: ctx.now,
        volumes,
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
    fn storage_snapshot_returns_volumes() {
        let snapshot = parse_storage_snapshot(&fixed_ctx())
            .expect("storage snapshot should succeed");

        // Should have at least one filesystem (root /)
        assert!(
            !snapshot.volumes.is_empty(),
            "should have at least one filesystem"
        );
    }

    #[test]
    fn root_filesystem_present() {
        let snapshot = parse_storage_snapshot(&fixed_ctx())
            .expect("storage snapshot should succeed");

        // Find root filesystem
        let root = snapshot
            .volumes
            .iter()
            .find(|v| v.mount_point == "/");

        assert!(root.is_some(), "root filesystem (/) should be present");

        if let Some(root) = root {
            // Root should have valid capacity
            match root.capacity_bytes {
                MetricValue::Supported { value } => {
                    assert!(value > 0, "root capacity should be > 0, got {}", value);
                }
                other => panic!("root capacity should be Supported, got {:?}", other),
            }

            // Root should have valid available
            match root.available_bytes {
                MetricValue::Supported { value } => {
                    assert!(
                        value > 0,
                        "root available should be > 0 (unless disk is completely full), got {}",
                        value
                    );
                }
                other => panic!("root available should be Supported, got {:?}", other),
            }
        }
    }

    #[test]
    fn no_pseudo_filesystems() {
        let snapshot = parse_storage_snapshot(&fixed_ctx())
            .expect("storage snapshot should succeed");

        // Verify no pseudo filesystems in results
        for volume in &snapshot.volumes {
            assert!(
                !is_pseudo_filesystem(&volume.filesystem),
                "pseudo filesystem should be filtered: {}",
                volume.filesystem
            );
        }
    }

    #[test]
    fn capacity_accounting_is_consistent() {
        let snapshot = parse_storage_snapshot(&fixed_ctx())
            .expect("storage snapshot should succeed");

        for volume in &snapshot.volumes {
            let capacity = match volume.capacity_bytes {
                MetricValue::Supported { value } => value,
                _ => continue, // Skip if not supported
            };
            let free = match volume.free_bytes {
                MetricValue::Supported { value } => value,
                _ => continue,
            };
            let available = match volume.available_bytes {
                MetricValue::Supported { value } => value,
                _ => continue,
            };

            // available should be <= free (available is what unprivileged users can use)
            assert!(
                available <= free,
                "available ({}) should be <= free ({}) for {}",
                available,
                free,
                volume.mount_point
            );

            // free should be <= capacity
            assert!(
                free <= capacity,
                "free ({}) should be <= capacity ({}) for {}",
                free,
                capacity,
                volume.mount_point
            );
        }
    }

    #[test]
    fn io_metrics_unavailable_in_u97() {
        let snapshot = parse_storage_snapshot(&fixed_ctx())
            .expect("storage snapshot should succeed");

        // All volumes should have I/O metrics marked as unavailable
        for volume in &snapshot.volumes {
            assert!(
                matches!(volume.read_bytes_per_sec, MetricValue::Unavailable { .. }),
                "read_bytes_per_sec should be unavailable in U97"
            );
            assert!(
                matches!(volume.write_bytes_per_sec, MetricValue::Unavailable { .. }),
                "write_bytes_per_sec should be unavailable in U97"
            );
            assert!(
                matches!(volume.read_ops_per_sec, MetricValue::Unavailable { .. }),
                "read_ops_per_sec should be unavailable in U97"
            );
            assert!(
                matches!(volume.write_ops_per_sec, MetricValue::Unavailable { .. }),
                "write_ops_per_sec should be unavailable in U97"
            );
            assert!(
                matches!(volume.io_latency_ms, MetricValue::Unavailable { .. }),
                "io_latency_ms should be unavailable in U97"
            );
        }
    }

    #[test]
    fn filesystem_names_are_valid() {
        let snapshot = parse_storage_snapshot(&fixed_ctx())
            .expect("storage snapshot should succeed");

        for volume in &snapshot.volumes {
            // Filesystem type should not be empty
            assert!(
                !volume.filesystem.is_empty(),
                "filesystem type should not be empty for {}",
                volume.mount_point
            );

            // Mount point should start with /
            assert!(
                volume.mount_point.starts_with('/'),
                "mount point should start with /, got: {}",
                volume.mount_point
            );

            // Device should not be empty
            assert!(
                !volume.device.is_empty(),
                "device should not be empty for {}",
                volume.mount_point
            );
        }
    }
}
