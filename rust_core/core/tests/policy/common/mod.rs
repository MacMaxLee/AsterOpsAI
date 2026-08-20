//! Shared test-only helpers for the policy/action pipeline integration
//! tests. **These are the only `ActionKind`/`TargetVerifier` real
//! implementations anywhere in this workspace** — see
//! `core::policy`/`core::actions`'s own module docs for why: real, shipped
//! implementations are unit U8's job. `std::process::Command` use here is
//! the sanctioned `core/tests/` exception in
//! `scripts/check-no-command-outside-exec.sh`, the same one U4's
//! PostgreSQL test harness already uses.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not test files.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use ai_ops_core::actions::{ActionContext, ActionError, ActionKind, TargetVerifier};
use ai_ops_core::policy::{
    ActionRequest, ActionTypeEntry, ActionTypeRegistry, RiskLevel, TargetIdentity,
};
use serde_json::Value;

/// A real `PlatformAdapter` for tests that need an `ActionContext` but
/// (unlike `actions_host_priority_affinity_test.rs`) don't actually
/// exercise a real host action — `test_registry`'s/`lifecycle_test_
/// registry`'s `construct` fns all ignore the context, same as before this
/// unit added the parameter.
pub fn test_action_context() -> ActionContext {
    ActionContext {
        platform: Arc::from(platform::current_platform_adapter()),
    }
}

/// Mirrors `core::telemetry::process`'s own `/proc/[pid]/stat` field-22
/// parsing (that code is Linux-gated, private, and lives in a different
/// crate module tree) — a small, self-contained, test-only reimplementation
/// rather than a cross-module dependency for ~10 lines of parsing.
fn read_start_time_ticks(pid: u32) -> Option<u64> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let close = raw.rfind(')')?;
    let rest: Vec<&str> = raw[close + 1..].split_whitespace().collect();
    // pid=1, comm=2, state=3 (rest[0]), ..., starttime=22 -> rest[22-3].
    rest.get(22 - 3)?.parse::<u64>().ok()
}

/// Not just directory existence: a killed child that hasn't been reaped by
/// its parent (this test process, via `wait()`) shows up as a zombie ('Z'
/// state) with its `/proc/[pid]` entry still present — real, but no longer
/// "alive" in the sense `verify()` cares about. Reads the state character
/// (`/proc/[pid]/stat` field 3, right after the `)` that closes `comm`).
pub fn process_is_alive(pid: u32) -> bool {
    let Ok(raw) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    let Some(close) = raw.rfind(')') else {
        return false;
    };
    !matches!(
        raw[close + 1..].trim_start().chars().next(),
        None | Some('Z')
    )
}

/// A real, disposable, test-owned child process (`sleep 300`) — the "real
/// target already existing in the world" that a proposed kill-process
/// action would name. Kept alive by the test (not by `TestActionKind`,
/// which only ever knows the target's `pid`/`start_time_ticks`, the same
/// way a real action wouldn't own a `Child` handle for a pre-existing
/// process either) so it can be reaped without a zombie even in a test
/// case that never reaches `apply()`.
pub struct SpawnedTarget {
    pub pid: u32,
    pub start_time_ticks: u64,
    child: Child,
}

impl SpawnedTarget {
    pub fn spawn() -> Self {
        // Stdio must NOT be inherited: an inherited stdout/stderr fd keeps
        // the test harness's own output pipe open for as long as this
        // child lives (300s), hanging anything reading that pipe (e.g.
        // `cargo test ... | tail`) well past the test process's own exit.
        let child = Command::new("sleep")
            .arg("300")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn disposable test target process");
        let pid = child.id();
        let start_time_ticks =
            read_start_time_ticks(pid).expect("read /proc/[pid]/stat for the just-spawned target");
        Self {
            pid,
            start_time_ticks,
            child,
        }
    }

    pub fn target(&self) -> TargetIdentity {
        TargetIdentity::Process {
            pid: self.pid,
            start_time_ticks: self.start_time_ticks,
        }
    }

    pub fn is_alive(&self) -> bool {
        process_is_alive(self.pid)
    }

    /// Explicit cleanup at the end of a passing test — same effect as
    /// `Drop` below, called by name so a test's happy path reads as
    /// deliberate cleanup rather than relying on implicit scope exit.
    pub fn ensure_reaped(mut self) {
        if self.is_alive() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

/// A panicking assertion before `ensure_reaped()` runs must not leak the
/// spawned process — same precedent as `core/tests/dbms/common/mod.rs`'s
/// `TestPostgres` Drop-based cleanup.
impl Drop for SpawnedTarget {
    fn drop(&mut self) {
        if self.is_alive() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

/// Kills a real, already-existing target by `pid` — via the real `kill`
/// command, not a `Child` handle it owns (a real "kill an arbitrary
/// process" action wouldn't have spawned its own target either).
pub struct TestActionKind {
    pid: u32,
    start_time_ticks: u64,
}

impl TestActionKind {
    pub fn for_target(pid: u32, start_time_ticks: u64) -> Self {
        Self {
            pid,
            start_time_ticks,
        }
    }
}

impl ActionKind for TestActionKind {
    fn action_type(&self) -> &'static str {
        "test.kill_process"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn capture_previous_state(&self) -> Result<Value, ActionError> {
        Ok(serde_json::json!({ "pid": self.pid, "state": "running" }))
    }

    fn apply(&self) -> Result<(), ActionError> {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(self.pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| ActionError::Apply(e.to_string()))?;
        if !status.success() {
            return Err(ActionError::Apply(format!(
                "kill -TERM exited with {status}"
            )));
        }
        for _ in 0..50 {
            if !process_is_alive(self.pid) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    fn verify(&self) -> Result<(), ActionError> {
        if process_is_alive(self.pid) {
            return Err(ActionError::Verify(
                "process is still alive after kill".to_string(),
            ));
        }
        Ok(())
    }

    fn rollback(&self, _previous_state: &Value) -> Result<(), ActionError> {
        Err(ActionError::Rollback(
            "killing a process is not reversible".to_string(),
        ))
    }
}

fn construct_test_kill_process(
    request: &ActionRequest,
    _context: &ActionContext,
) -> Result<Box<dyn ActionKind>, String> {
    match request.target {
        TargetIdentity::Process {
            pid,
            start_time_ticks,
        } => Ok(Box::new(TestActionKind::for_target(pid, start_time_ticks))),
        TargetIdentity::DbSession { .. } => {
            Err("test.kill_process requires a Process target".to_string())
        }
    }
}

fn validate_no_parameters(_params: &Value) -> Result<(), String> {
    Ok(())
}

pub fn test_registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register(
        "test.kill_process",
        ActionTypeEntry {
            risk_level: RiskLevel::High,
            validate_parameters: validate_no_parameters,
            construct: construct_test_kill_process,
        },
    );
    registry
}

/// A trivially real (does nothing, but genuinely runs) `ActionKind` for
/// tests that exercise the policy/approval lifecycle mechanism itself
/// rather than a real mutation — never reaches `execute()` in those tests.
pub struct NoOpActionKind {
    risk_level: RiskLevel,
}

impl ActionKind for NoOpActionKind {
    fn action_type(&self) -> &'static str {
        "test.no_op"
    }

    fn risk_level(&self) -> RiskLevel {
        self.risk_level
    }

    fn capture_previous_state(&self) -> Result<Value, ActionError> {
        Ok(serde_json::json!({}))
    }

    fn apply(&self) -> Result<(), ActionError> {
        Ok(())
    }

    fn verify(&self) -> Result<(), ActionError> {
        Ok(())
    }

    fn rollback(&self, _previous_state: &Value) -> Result<(), ActionError> {
        Ok(())
    }
}

fn construct_auto_allow(
    _request: &ActionRequest,
    _context: &ActionContext,
) -> Result<Box<dyn ActionKind>, String> {
    Ok(Box::new(NoOpActionKind {
        risk_level: RiskLevel::Informational,
    }))
}

fn construct_needs_approval(
    _request: &ActionRequest,
    _context: &ActionContext,
) -> Result<Box<dyn ActionKind>, String> {
    Ok(Box::new(NoOpActionKind {
        risk_level: RiskLevel::Medium,
    }))
}

/// A registry for the approval-lifecycle/protected-resource tests: one
/// always-auto-allowed action type and one always-needs-approval action
/// type, both no-ops — those tests exercise `policy::evaluate`/
/// `policy::approval`, not `actions::execute`.
pub fn lifecycle_test_registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    registry.register(
        "test.auto_allow",
        ActionTypeEntry {
            risk_level: RiskLevel::Informational,
            validate_parameters: validate_no_parameters,
            construct: construct_auto_allow,
        },
    );
    registry.register(
        "test.needs_approval",
        ActionTypeEntry {
            risk_level: RiskLevel::Medium,
            validate_parameters: validate_no_parameters,
            construct: construct_needs_approval,
        },
    );
    registry
}

/// Re-reads the live process's real `/proc/[pid]/stat` field 22 and
/// compares it to `expected` — a real re-verification, not a stand-in.
pub struct ProcessTargetVerifier;

impl TargetVerifier for ProcessTargetVerifier {
    fn verify(&self, expected: &TargetIdentity) -> Result<(), ActionError> {
        let TargetIdentity::Process {
            pid,
            start_time_ticks,
        } = *expected
        else {
            return Err(ActionError::TargetChanged);
        };
        match read_start_time_ticks(pid) {
            Some(live) if live == start_time_ticks => Ok(()),
            _ => Err(ActionError::TargetChanged),
        }
    }
}

/// A verifier that always reports the target changed — for tests proving
/// the executor aborts before `apply()`/`rollback()` ever run.
pub struct AlwaysMismatchVerifier;

impl TargetVerifier for AlwaysMismatchVerifier {
    fn verify(&self, _expected: &TargetIdentity) -> Result<(), ActionError> {
        Err(ActionError::TargetChanged)
    }
}
