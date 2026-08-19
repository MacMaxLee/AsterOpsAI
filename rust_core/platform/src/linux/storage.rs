//! Per-volume capacity/free-space via `statvfs(2)`. This is a deliberate,
//! documented exception to the `ProcSource`-based parsing path: capacity
//! isn't exposed by any `/proc`/`/sys` text file, only by this syscall —
//! same treatment as `getrusage`-based self-metrics in `linux/mod.rs`:
//! live-only, not fixture-tested.

use std::ffi::CString;
use std::mem::MaybeUninit;

use crate::error::CapabilityError;

#[derive(Debug, Clone, Copy)]
pub struct VolumeCapacity {
    pub capacity_bytes: u64,
    pub free_bytes: u64,
    /// Space available to an unprivileged user (`f_bavail`), which may be
    /// lower than `free_bytes` on filesystems with reserved blocks.
    pub available_bytes: u64,
}

#[allow(unsafe_code)]
pub fn volume_capacity(mount_point: &str) -> Result<VolumeCapacity, CapabilityError> {
    let path = CString::new(mount_point).map_err(|_| {
        CapabilityError::Unavailable(format!(
            "mount point {mount_point:?} contains an interior NUL byte"
        ))
    })?;

    let mut stat = MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `path` is a valid NUL-terminated C string for the duration of
    // this call; `stat` is a valid, correctly-sized, writable buffer that
    // `statvfs` only ever writes to, and we check the return code before
    // treating it as initialized.
    let rc = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return Err(CapabilityError::Io(std::io::Error::last_os_error()));
    }
    // SAFETY: `rc == 0` guarantees the kernel fully populated `stat`.
    let stat = unsafe { stat.assume_init() };

    let block_size: u64 = stat.f_frsize;
    Ok(VolumeCapacity {
        capacity_bytes: stat.f_blocks * block_size,
        free_bytes: stat.f_bfree * block_size,
        available_bytes: stat.f_bavail * block_size,
    })
}
