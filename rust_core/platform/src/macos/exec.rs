//! The only module (with its `linux`/`windows` siblings) permitted to call
//! `std::process::Command` — enforced by
//! `scripts/check-no-command-outside-exec.sh`.
//!
//! This module serves as the single, CI-enforced location for all
//! command execution on macOS. Future telemetry units (U95-U100) will add
//! wrappers here for parsing system commands like `ps`, `netstat`, `iostat`,
//! `sysctl`, etc.
//!
//! Established in unit U94 as a foundation; expanded in later units as needed.

use std::process::Command;

/// Executes `ps -o state= -p [pid]` to get process state.
/// Returns the raw state output string, or an error if ps fails.
pub(crate) fn get_process_state(pid: u32) -> std::io::Result<String> {
    let output = Command::new("ps")
        .arg("-o")
        .arg("state=")
        .arg("-p")
        .arg(pid.to_string())
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("process {} not found", pid),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Executes `netstat -ibn` to get network interface statistics.
/// Returns the raw netstat output string, or an error if netstat fails.
pub fn get_netstat_interfaces() -> std::io::Result<String> {
    let output = Command::new("netstat")
        .arg("-ibn")
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("netstat -ibn failed with status: {}", output.status),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_command_works() {
        // Basic smoke test: verify Command works and we can execute /bin/echo
        let output = Command::new("/bin/echo")
            .arg("hello")
            .output()
            .expect("failed to execute /bin/echo");

        assert!(output.status.success(), "echo command should succeed");

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("hello"),
            "echo output should contain 'hello', got: {:?}",
            stdout
        );
    }
}
