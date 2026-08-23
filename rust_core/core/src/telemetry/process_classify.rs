//! SRS FR-PRO-003: deterministic process classification. Pure and I/O-free
//! by design — testable with synthetic inputs, independent of the
//! `ProcSource`/fixture machinery entirely.

use contracts::telemetry::ProcessCategory;

const KNOWN_SYSTEM_COMMS: &[&str] = &[
    "systemd",
    "init",
    "kthreadd",
    "udevd",
    "systemd-journald",
    "systemd-udevd",
    "systemd-logind",
];
const KNOWN_DBMS_COMMS: &[&str] = &["postgres", "postmaster", "mysqld", "mariadbd"];

pub struct ProcessClassifyInput<'a> {
    pub comm: &'a str,
    /// Not populated by `parse_process_snapshot` in unit U1 (see the ADR/
    /// plan note on why `/proc/pid/exe` isn't read); kept so this function
    /// is directly testable with full-fidelity synthetic inputs.
    pub exe_path: Option<&'a str>,
    pub uid: u32,
    pub cgroup_path: &'a str,
    /// True when `/proc/[pid]/cmdline` was empty. This — not a "[bracketed]"
    /// comm, which is only a `ps`/`top` *display* convention and never
    /// appears in the raw `/proc/[pid]/stat` field — is the real signal for
    /// a kernel thread (confirmed against a live kernel: `/proc/2/stat`'s
    /// comm is literally `kthreadd`, no brackets).
    pub cmdline_is_empty: bool,
}

pub fn classify_process(input: &ProcessClassifyInput<'_>) -> ProcessCategory {
    let comm = input.comm.trim();

    if input.cmdline_is_empty && input.uid == 0 {
        return ProcessCategory::System;
    }

    let exe_name = input
        .exe_path
        .and_then(|p| p.rsplit('/').next())
        .unwrap_or("");

    if KNOWN_DBMS_COMMS.contains(&comm) || KNOWN_DBMS_COMMS.contains(&exe_name) {
        return ProcessCategory::DbmsEngine;
    }

    if input.uid == 0 && KNOWN_SYSTEM_COMMS.contains(&comm) {
        return ProcessCategory::System;
    }

    // Checked before the generic ".service" rule below: a user's own
    // session cgroup nearly always contains a "user@<uid>.service" segment
    // (systemd's per-user manager instance) as a substring, which is not
    // itself a background service — confirmed against a live desktop shell
    // whose real cgroup was
    // ".../user@1000.service/app.slice/app-org.gnome.Terminal.slice/...".
    // Checking "app.slice"/"user.slice"/"user@" first routes real user
    // sessions correctly regardless of where ".service" appears in the path.
    if input.cgroup_path.contains("app.slice")
        || input.cgroup_path.contains("user.slice")
        || input.cgroup_path.contains("user@")
    {
        return ProcessCategory::UserApplication;
    }

    if input.cgroup_path.contains(".service") {
        return ProcessCategory::BackgroundService;
    }

    ProcessCategory::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SRS FR-PRO-003: fixed-rule-set classification into a deterministic
    /// category enum, unit-testable without a live process table.
    #[test]
    fn kernel_thread() {
        // Real signal confirmed against a live kernel: kthreadd (pid 2) has
        // comm "kthreadd" (no brackets), uid 0, empty cmdline, cgroup "/".
        let input = ProcessClassifyInput {
            comm: "kthreadd",
            exe_path: None,
            uid: 0,
            cgroup_path: "/",
            cmdline_is_empty: true,
        };
        assert_eq!(classify_process(&input), ProcessCategory::System);
    }

    #[test]
    fn system_daemon() {
        let input = ProcessClassifyInput {
            comm: "systemd",
            exe_path: Some("/usr/lib/systemd/systemd"),
            uid: 0,
            cgroup_path: "/init.scope",
            cmdline_is_empty: false,
        };
        assert_eq!(classify_process(&input), ProcessCategory::System);
    }

    #[test]
    fn dbms_engine_by_comm() {
        let input = ProcessClassifyInput {
            comm: "postgres",
            exe_path: None,
            uid: 999,
            cgroup_path: "/system.slice/postgresql.service",
            cmdline_is_empty: false,
        };
        assert_eq!(classify_process(&input), ProcessCategory::DbmsEngine);
    }

    #[test]
    fn background_service() {
        let input = ProcessClassifyInput {
            comm: "nginx",
            exe_path: Some("/usr/sbin/nginx"),
            uid: 33,
            cgroup_path: "/system.slice/nginx.service",
            cmdline_is_empty: false,
        };
        assert_eq!(classify_process(&input), ProcessCategory::BackgroundService);
    }

    #[test]
    fn user_application() {
        let input = ProcessClassifyInput {
            comm: "gnome-terminal",
            exe_path: None,
            uid: 1000,
            cgroup_path: "/user.slice/user-1000.slice/user@1000.service/app.slice/vte.scope",
            cmdline_is_empty: false,
        };
        assert_eq!(classify_process(&input), ProcessCategory::UserApplication);
    }

    #[test]
    fn unknown_fallback() {
        let input = ProcessClassifyInput {
            comm: "mystery",
            exe_path: None,
            uid: 5000,
            cgroup_path: "/some/unrecognized/path",
            cmdline_is_empty: false,
        };
        assert_eq!(classify_process(&input), ProcessCategory::Unknown);
    }
}
