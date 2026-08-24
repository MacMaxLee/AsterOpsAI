//! Process priority control for macOS using POSIX setpriority(2)/getpriority(2).
//!
//! Unlike Linux's implementation which reads `/proc/[pid]/stat` to avoid
//! getpriority's errno ambiguity, macOS has no /proc filesystem, so we use
//! getpriority directly with proper errno handling (clear errno before call,
//! check it after, since -1 is both a valid nice value and an error sentinel).

use crate::adapter::ProcessPriority;
use crate::error::CapabilityError;

/// Maps errno to appropriate CapabilityError variant.
fn map_last_os_error() -> CapabilityError {
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EPERM) => CapabilityError::PermissionRequired(
            "insufficient privilege to change process priority".to_string(),
        ),
        Some(libc::ESRCH) => CapabilityError::NotFound(format!("no such process: {err}")),
        Some(libc::EINVAL) => CapabilityError::InvalidInput(format!("invalid argument: {err}")),
        _ => CapabilityError::Io(err),
    }
}

/// Maps nice value (−20 to 19) to ProcessPriority enum.
fn nice_to_priority(nice: i32) -> ProcessPriority {
    // Same mapping as Linux implementation
    if nice >= 15 {
        ProcessPriority::Idle
    } else if nice >= 5 {
        ProcessPriority::BelowNormal
    } else if nice > -5 {
        ProcessPriority::Normal
    } else if nice > -10 {
        ProcessPriority::AboveNormal
    } else {
        ProcessPriority::High
    }
}

/// Maps ProcessPriority enum to nice value (−20 to 19).
fn priority_to_nice(priority: ProcessPriority) -> i32 {
    match priority {
        ProcessPriority::Idle => 20,
        ProcessPriority::BelowNormal => 10,
        ProcessPriority::Normal => 0,
        ProcessPriority::AboveNormal => -10,
        ProcessPriority::High => -20,
    }
}

/// Gets the scheduling priority (nice value) of a process.
///
/// # SAFETY (documented at call site below)
/// getpriority(2) returns -1 both as a valid nice value and as an error
/// sentinel, so we must clear errno before the call and check it after.
#[allow(unsafe_code)]
pub fn get_priority(pid: u32) -> Result<ProcessPriority, CapabilityError> {
    // Clear errno before calling getpriority (POSIX requirement)
    unsafe {
        *libc::__error() = 0;
    }

    // SAFETY: `pid` is a plain integer; `getpriority` performs no pointer
    // dereference. We check errno afterward to distinguish -1-as-value from
    // -1-as-error.
    let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid) };

    // Check if error occurred (errno != 0 means error)
    let errno = unsafe { *libc::__error() };
    if errno != 0 {
        return Err(map_last_os_error());
    }

    Ok(nice_to_priority(nice))
}

/// Sets the scheduling priority (nice value) of a process.
///
/// # SAFETY (documented at call site below)
/// setpriority(2)'s 0/−1 return is unambiguous — no errno-clearing needed.
#[allow(unsafe_code)]
pub fn set_priority(pid: u32, priority: ProcessPriority) -> Result<(), CapabilityError> {
    let nice = priority_to_nice(priority);

    // SAFETY: `pid` and `nice` are plain integers; `setpriority` performs no
    // pointer dereference on this process's memory. Return value is
    // unambiguous (0 = success, -1 = error).
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) };

    if rc != 0 {
        return Err(map_last_os_error());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_priority_of_self_is_a_real_value() {
        let pid = std::process::id();
        let priority = get_priority(pid).expect("should be able to read own priority");

        // Should be one of the five valid priority levels
        assert!(
            matches!(
                priority,
                ProcessPriority::Idle
                    | ProcessPriority::BelowNormal
                    | ProcessPriority::Normal
                    | ProcessPriority::AboveNormal
                    | ProcessPriority::High
            ),
            "got unexpected priority: {:?}",
            priority
        );
    }

    #[test]
    fn lowering_priority_succeeds_unprivileged() {
        let pid = std::process::id();

        // Get current priority
        let original = get_priority(pid).expect("should read current priority");

        // Try to lower priority (increase nice value) — should succeed unprivileged
        let result = set_priority(pid, ProcessPriority::BelowNormal);

        // Restore original priority regardless of test outcome
        let _ = set_priority(pid, original);

        // Should have succeeded (lowering priority doesn't need privilege)
        result.expect("lowering priority should succeed unprivileged");
    }

    #[test]
    fn raising_priority_fails_unprivileged() {
        let pid = std::process::id();

        // Try to raise priority (decrease nice value) — should fail unprivileged
        let result = set_priority(pid, ProcessPriority::High);

        // Should fail with PermissionRequired
        match result {
            Err(CapabilityError::PermissionRequired(_)) => {
                // Expected: unprivileged process can't raise priority
            }
            Ok(_) => {
                panic!("raising priority should fail unprivileged (are you running as root?)");
            }
            Err(other) => {
                panic!("unexpected error when raising priority: {:?}", other);
            }
        }
    }

    #[test]
    fn get_priority_nonexistent_process_fails() {
        // PID 99999 is very unlikely to exist
        let result = get_priority(99999);

        // Should fail with NotFound or Unavailable
        assert!(
            result.is_err(),
            "reading priority of nonexistent process should fail"
        );
    }
}
