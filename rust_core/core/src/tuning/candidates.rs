//! Live-state diff -> candidate `ActionRequest`s. Reads the target's actual
//! current priority/affinity via `ActionContext.platform` (the same
//! `PlatformAdapter` trait U8's real actions use — a small, self-contained
//! JSON encoding matches `core::actions::host::{priority,affinity}`'s own
//! wire shape without depending on that Linux-gated module, mirroring
//! `core/tests/policy/common/mod.rs`'s own "small reimplementation over a
//! cross-module dependency" precedent) and proposes an action only for a
//! field that's actually different from the desired state — never a
//! no-op action nobody asked for.

use platform::{CpuAffinityMask, ProcessPriority};
use serde_json::{json, Value};

use crate::actions::ActionContext;
use crate::policy::{ActionRequest, ResourceDescriptor, TargetIdentity};

use super::error::TuningError;
use super::profile::DesiredState;

fn priority_param(priority: ProcessPriority) -> Value {
    let s = match priority {
        ProcessPriority::Idle => "IDLE",
        ProcessPriority::BelowNormal => "BELOW_NORMAL",
        ProcessPriority::Normal => "NORMAL",
        ProcessPriority::AboveNormal => "ABOVE_NORMAL",
        ProcessPriority::High => "HIGH",
    };
    json!({ "priority": s })
}

fn affinity_param(mask: &CpuAffinityMask) -> Value {
    json!({ "cpus": mask.cpus.iter().collect::<Vec<_>>() })
}

pub fn build_candidates(
    desired: &DesiredState,
    target: TargetIdentity,
    resource: &ResourceDescriptor,
    context: &ActionContext,
    requested_by: &str,
) -> Result<Vec<ActionRequest>, TuningError> {
    let TargetIdentity::Process { pid, .. } = target else {
        return Err(TuningError::UnsupportedTarget);
    };

    let mut candidates = Vec::new();

    if let Some(want) = desired.priority {
        let live = context
            .platform
            .get_process_priority(pid)
            .map_err(|e| TuningError::CapabilityUnavailable(e.to_string()))?;
        if live != want {
            candidates.push(ActionRequest {
                action_type: "host.set_process_priority".to_string(),
                target,
                resource: resource.clone(),
                parameters: priority_param(want),
                requested_by: requested_by.to_string(),
            });
        }
    }

    if let Some(want) = &desired.cpu_affinity {
        let live = context
            .platform
            .get_process_cpu_affinity(pid)
            .map_err(|e| TuningError::CapabilityUnavailable(e.to_string()))?;
        if &live != want {
            candidates.push(ActionRequest {
                action_type: "host.set_process_cpu_affinity".to_string(),
                target,
                resource: resource.clone(),
                parameters: affinity_param(want),
                requested_by: requested_by.to_string(),
            });
        }
    }

    Ok(candidates)
}
