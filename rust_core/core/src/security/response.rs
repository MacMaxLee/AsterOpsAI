//! Unit U11's one real security response action (SRS FR-SEC-004),
//! Linux-only for now — same precedent and same reasoning as
//! `actions::host` (unit U8): `platform::{windows,macos}` stub the
//! underlying capability until unit U12, so registering this action type
//! elsewhere would just mean every real call fails at
//! `check_capability()`.
//!
//! **FR-SEC-004's closed-enum guarantee**: [`SecurityResponseAction`] is
//! the *only* sanctioned way a security-response-flagged action type gets
//! registered — its `action_type()` match is exhaustive over the enum, so
//! adding a firewall/AV-disabling variant would require deliberately
//! touching this exact enum and this exact match, not a string literal
//! typed somewhere else. `BLOCK_DEVICE` (TRS §37's other named example)
//! is deliberately not a variant yet — it needs a `TargetIdentity::Device`
//! variant that doesn't exist (see docs/adr/0016).

use std::sync::Arc;

use platform::PlatformAdapter;
use serde_json::Value;

use crate::actions::{ActionContext, ActionError, ActionKind};
use crate::policy::{ActionRequest, ActionTypeEntry, RiskLevel, TargetIdentity};

/// The closed set (SRS FR-SEC-004). Exactly one variant today —
/// deliberately not padded with a placeholder second one; see this
/// module's own doc comment for why `BLOCK_DEVICE` isn't here yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityResponseAction {
    SuspendProcess,
}

impl SecurityResponseAction {
    pub fn action_type(self) -> &'static str {
        match self {
            Self::SuspendProcess => "security.suspend_process",
        }
    }
}

/// Real: `SIGSTOP` via `PlatformAdapter::suspend_process`, verified via
/// `is_process_stopped`; rollback is `SIGCONT` via `resume_process`. TRS
/// §37: "always require approval" — `risk_level` is `High`, which
/// `policy::risk::decide` maps to `RequireApproval` in every environment,
/// never `AutoAllow`.
pub struct SuspendProcessAction {
    platform: Arc<dyn PlatformAdapter>,
    pid: u32,
}

impl SuspendProcessAction {
    pub fn new(platform: Arc<dyn PlatformAdapter>, pid: u32) -> Self {
        Self { platform, pid }
    }

    pub fn from_request(
        platform: Arc<dyn PlatformAdapter>,
        request: &ActionRequest,
    ) -> Result<Self, String> {
        let TargetIdentity::Process { pid, .. } = request.target else {
            return Err("security.suspend_process requires a Process target".to_string());
        };
        Ok(Self::new(platform, pid))
    }
}

impl ActionKind for SuspendProcessAction {
    fn action_type(&self) -> &'static str {
        SecurityResponseAction::SuspendProcess.action_type()
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn is_security_response(&self) -> bool {
        true
    }

    fn capture_previous_state(&self) -> Result<Value, ActionError> {
        // Rollback is unconditionally "resume" regardless of the prior
        // state — nothing meaningful to capture, same precedent
        // `NoOpActionKind` (core/tests/policy/common/mod.rs) already uses.
        Ok(serde_json::json!({}))
    }

    /// A real discovery from testing against a real spawned process:
    /// unlike `setpriority`/`sched_setaffinity` (synchronous syscalls,
    /// unit U8), `SIGSTOP` delivery is asynchronous — the kernel has to
    /// actually schedule the target before its `/proc/[pid]/stat` state
    /// reflects `T`. A bare `kill()` returning `0` only means the signal
    /// was *sent*, not that the process has genuinely stopped yet. Polls
    /// briefly for the real transition, the same real-completion-polling
    /// precedent `core/tests/policy/common/mod.rs`'s own `TestActionKind::
    /// apply()` already uses after its own `kill -TERM`.
    fn apply(&self) -> Result<(), ActionError> {
        self.platform
            .suspend_process(self.pid)
            .map_err(|e| ActionError::Apply(e.to_string()))?;
        for _ in 0..50 {
            if self
                .platform
                .is_process_stopped(self.pid)
                .map_err(|e| ActionError::Apply(e.to_string()))?
            {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Err(ActionError::Apply(
            "process did not stop within the expected time after SIGSTOP".to_string(),
        ))
    }

    fn verify(&self) -> Result<(), ActionError> {
        let stopped = self
            .platform
            .is_process_stopped(self.pid)
            .map_err(|e| ActionError::Verify(e.to_string()))?;
        if !stopped {
            return Err(ActionError::Verify(
                "process is not stopped after suspend".to_string(),
            ));
        }
        Ok(())
    }

    /// Same real async-signal-delivery reasoning as `apply()`'s own doc
    /// comment: `SIGCONT`'s effect on `/proc/[pid]/stat`'s state isn't
    /// instantaneous either.
    fn rollback(&self, _previous_state: &Value) -> Result<(), ActionError> {
        self.platform
            .resume_process(self.pid)
            .map_err(|e| ActionError::Rollback(e.to_string()))?;
        for _ in 0..50 {
            if !self
                .platform
                .is_process_stopped(self.pid)
                .map_err(|e| ActionError::Rollback(e.to_string()))?
            {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Err(ActionError::Rollback(
            "process did not resume within the expected time after SIGCONT".to_string(),
        ))
    }
}

fn construct_suspend_process(
    request: &ActionRequest,
    context: &ActionContext,
) -> Result<Box<dyn ActionKind>, String> {
    SuspendProcessAction::from_request(Arc::clone(&context.platform), request)
        .map(|action| Box::new(action) as Box<dyn ActionKind>)
}

fn validate_no_parameters(_params: &Value) -> Result<(), String> {
    Ok(())
}

pub fn suspend_process_entry() -> ActionTypeEntry {
    ActionTypeEntry {
        risk_level: RiskLevel::High,
        // Real: SIGCONT unconditionally undoes SIGSTOP, no privilege
        // asymmetry (unlike ADR 0013's priority finding).
        reversible: true,
        validate_parameters: validate_no_parameters,
        construct: construct_suspend_process,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SRS FR-SEC-004: the closed-enum guarantee itself — this module's
    /// own doc comment explains why a dynamic test can only pin what the
    /// compiler's exhaustive match already enforces (adding a firewall/
    /// AV-disabling variant requires touching this exact enum), but a pin
    /// test still catches an accidental action-string change going
    /// unnoticed, which a match's exhaustiveness alone wouldn't.
    #[test]
    fn suspend_process_is_the_only_security_response_action_type() {
        assert_eq!(
            SecurityResponseAction::SuspendProcess.action_type(),
            "security.suspend_process"
        );
    }
}
