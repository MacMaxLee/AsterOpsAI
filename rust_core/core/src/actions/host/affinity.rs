//! `host.set_process_cpu_affinity` — TRS §38's other named "U8 action"
//! (`SetProcessAffinityMask` on Windows; real Linux implementation via
//! `sched_setaffinity(2)`, `platform::linux::process_control`).

use std::collections::BTreeSet;
use std::sync::Arc;

use platform::{CpuAffinityMask, PlatformAdapter};
use serde_json::Value;

use crate::actions::{ActionError, ActionKind};
use crate::policy::{ActionRequest, RiskLevel, TargetIdentity};

pub(crate) fn affinity_to_json(mask: &CpuAffinityMask) -> Value {
    serde_json::json!({ "cpus": mask.cpus.iter().collect::<Vec<_>>() })
}

fn json_to_affinity(value: &Value) -> Option<CpuAffinityMask> {
    let cpus: BTreeSet<usize> = value
        .get("cpus")?
        .as_array()?
        .iter()
        .map(|v| v.as_u64().map(|n| n as usize))
        .collect::<Option<BTreeSet<usize>>>()?;
    Some(CpuAffinityMask { cpus })
}

pub(crate) fn parse_affinity_param(params: &Value) -> Result<CpuAffinityMask, String> {
    json_to_affinity(params)
        .ok_or_else(|| "expected {\"cpus\": [<non-negative integers>, ...]}".to_string())
}

pub struct SetProcessCpuAffinityAction {
    platform: Arc<dyn PlatformAdapter>,
    pid: u32,
    requested: CpuAffinityMask,
}

impl SetProcessCpuAffinityAction {
    pub fn new(platform: Arc<dyn PlatformAdapter>, pid: u32, requested: CpuAffinityMask) -> Self {
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
            return Err("host.set_process_cpu_affinity requires a Process target".to_string());
        };
        let requested = parse_affinity_param(&request.parameters)?;
        Ok(Self::new(platform, pid, requested))
    }
}

impl ActionKind for SetProcessCpuAffinityAction {
    fn action_type(&self) -> &'static str {
        "host.set_process_cpu_affinity"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn check_capability(&self) -> Result<(), ActionError> {
        self.platform
            .get_process_cpu_affinity(self.pid)
            .map(|_| ())
            .map_err(|e| ActionError::CapabilityUnavailable(e.to_string()))
    }

    fn capture_previous_state(&self) -> Result<Value, ActionError> {
        let current = self
            .platform
            .get_process_cpu_affinity(self.pid)
            .map_err(|e| ActionError::CapturePreviousState(e.to_string()))?;
        Ok(affinity_to_json(&current))
    }

    fn apply(&self) -> Result<(), ActionError> {
        self.platform
            .set_process_cpu_affinity(self.pid, &self.requested)
            .map_err(|e| ActionError::Apply(e.to_string()))
    }

    fn verify(&self) -> Result<(), ActionError> {
        let current = self
            .platform
            .get_process_cpu_affinity(self.pid)
            .map_err(|e| ActionError::Verify(e.to_string()))?;
        if current != self.requested {
            return Err(ActionError::Verify(format!(
                "expected affinity {:?}, observed {current:?}",
                self.requested
            )));
        }
        Ok(())
    }

    fn rollback(&self, previous_state: &Value) -> Result<(), ActionError> {
        let previous = json_to_affinity(previous_state).ok_or_else(|| {
            ActionError::Rollback("previous_state is not a valid affinity mask".to_string())
        })?;
        self.platform
            .set_process_cpu_affinity(self.pid, &previous)
            .map_err(|e| ActionError::Rollback(e.to_string()))
    }
}
