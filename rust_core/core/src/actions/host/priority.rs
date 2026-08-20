//! `host.set_process_priority` — one of TRS §38's "two U8 actions"
//! (`SetPriorityClass` on Windows; real Linux implementation via
//! `setpriority(2)`, `platform::linux::process_control`).

use std::sync::Arc;

use platform::{PlatformAdapter, ProcessPriority};
use serde_json::Value;

use crate::actions::{ActionError, ActionKind};
use crate::policy::{ActionRequest, RiskLevel, TargetIdentity};

pub(crate) fn priority_to_json(priority: ProcessPriority) -> Value {
    Value::String(priority_to_str(priority).to_string())
}

fn priority_to_str(priority: ProcessPriority) -> &'static str {
    match priority {
        ProcessPriority::Idle => "IDLE",
        ProcessPriority::BelowNormal => "BELOW_NORMAL",
        ProcessPriority::Normal => "NORMAL",
        ProcessPriority::AboveNormal => "ABOVE_NORMAL",
        ProcessPriority::High => "HIGH",
    }
}

fn str_to_priority(s: &str) -> Option<ProcessPriority> {
    match s {
        "IDLE" => Some(ProcessPriority::Idle),
        "BELOW_NORMAL" => Some(ProcessPriority::BelowNormal),
        "NORMAL" => Some(ProcessPriority::Normal),
        "ABOVE_NORMAL" => Some(ProcessPriority::AboveNormal),
        "HIGH" => Some(ProcessPriority::High),
        _ => None,
    }
}

pub(crate) fn parse_priority_param(params: &Value) -> Result<ProcessPriority, String> {
    let s = params
        .get("priority")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field \"priority\"".to_string())?;
    str_to_priority(s).ok_or_else(|| format!("unknown priority {s:?}"))
}

pub struct SetProcessPriorityAction {
    platform: Arc<dyn PlatformAdapter>,
    pid: u32,
    requested: ProcessPriority,
}

impl SetProcessPriorityAction {
    pub fn new(platform: Arc<dyn PlatformAdapter>, pid: u32, requested: ProcessPriority) -> Self {
        Self {
            platform,
            pid,
            requested,
        }
    }

    pub fn from_request(
        platform: Arc<dyn PlatformAdapter>,
        request: &ActionRequest,
    ) -> Result<Self, String> {
        let TargetIdentity::Process { pid, .. } = request.target else {
            return Err("host.set_process_priority requires a Process target".to_string());
        };
        let requested = parse_priority_param(&request.parameters)?;
        Ok(Self::new(platform, pid, requested))
    }
}

impl ActionKind for SetProcessPriorityAction {
    fn action_type(&self) -> &'static str {
        "host.set_process_priority"
    }

    fn risk_level(&self) -> RiskLevel {
        // One process, fully reversible via capture_previous_state/
        // rollback — a documented judgment call (docs/adr/0013), not
        // varied by which priority level was requested.
        RiskLevel::Low
    }

    fn check_capability(&self) -> Result<(), ActionError> {
        self.platform
            .get_process_priority(self.pid)
            .map(|_| ())
            .map_err(|e| ActionError::CapabilityUnavailable(e.to_string()))
    }

    fn capture_previous_state(&self) -> Result<Value, ActionError> {
        let current = self
            .platform
            .get_process_priority(self.pid)
            .map_err(|e| ActionError::CapturePreviousState(e.to_string()))?;
        Ok(priority_to_json(current))
    }

    fn apply(&self) -> Result<(), ActionError> {
        self.platform
            .set_process_priority(self.pid, self.requested)
            .map_err(|e| ActionError::Apply(e.to_string()))
    }

    fn verify(&self) -> Result<(), ActionError> {
        let current = self
            .platform
            .get_process_priority(self.pid)
            .map_err(|e| ActionError::Verify(e.to_string()))?;
        if current != self.requested {
            return Err(ActionError::Verify(format!(
                "expected priority {:?}, observed {current:?}",
                self.requested
            )));
        }
        Ok(())
    }

    fn rollback(&self, previous_state: &Value) -> Result<(), ActionError> {
        let s = previous_state
            .as_str()
            .ok_or_else(|| ActionError::Rollback("previous_state is not a string".to_string()))?;
        let previous = str_to_priority(s)
            .ok_or_else(|| ActionError::Rollback(format!("unknown priority {s:?}")))?;
        self.platform
            .set_process_priority(self.pid, previous)
            .map_err(|e| ActionError::Rollback(e.to_string()))
    }
}
