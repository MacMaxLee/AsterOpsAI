//! Unit U8's real, shipped host-level actions — the only `ActionKind`
//! implementations in `core/src` (everything else in `core::actions` is
//! framework, per `core::policy`'s own module doc). Both are Linux-only
//! for now: Windows/macOS have no `PlatformAdapter` implementation of the
//! underlying capability yet (`platform/{windows,macos}/mod.rs` stub with
//! `CapabilityError::Unsupported("...see unit U12")`), so registering
//! these action types on those platforms would just mean every real call
//! fails at `check_capability()` — harmless, but pointless; `service`
//! (wiring these into the registry, not this unit's own scope) should
//! gate registration on `cfg!(target_os = "linux")` until then.

pub mod affinity;
pub mod priority;
pub mod target_verifier;

pub use affinity::SetProcessCpuAffinityAction;
pub use priority::SetProcessPriorityAction;
pub use target_verifier::ProcessTargetVerifier;

use std::sync::Arc;

use crate::actions::{ActionContext, ActionKind};
use crate::policy::{ActionRequest, ActionTypeEntry, RiskLevel};

pub fn set_process_priority_entry() -> ActionTypeEntry {
    ActionTypeEntry {
        risk_level: RiskLevel::Low,
        // NOT reversible (unit U10, docs/adr/0015): ADR 0013 already
        // proved restoring a *lowered* nice value needs CAP_SYS_NICE —
        // rollback can genuinely fail without privilege, so this can't
        // honestly claim an unconditional round trip.
        reversible: false,
        validate_parameters: |params| priority::parse_priority_param(params).map(|_| ()),
        construct: construct_set_process_priority,
    }
}

fn construct_set_process_priority(
    request: &ActionRequest,
    context: &ActionContext,
) -> Result<Box<dyn ActionKind>, String> {
    SetProcessPriorityAction::from_request(Arc::clone(&context.platform), request)
        .map(|action| Box::new(action) as Box<dyn ActionKind>)
}

pub fn set_process_cpu_affinity_entry() -> ActionTypeEntry {
    ActionTypeEntry {
        risk_level: RiskLevel::Low,
        // Reversible (unit U10): U8's own end-to-end test already proves
        // a full unprivileged round trip, with no analogue of the
        // priority action's privilege asymmetry.
        reversible: true,
        validate_parameters: |params| affinity::parse_affinity_param(params).map(|_| ()),
        construct: construct_set_process_cpu_affinity,
    }
}

fn construct_set_process_cpu_affinity(
    request: &ActionRequest,
    context: &ActionContext,
) -> Result<Box<dyn ActionKind>, String> {
    SetProcessCpuAffinityAction::from_request(Arc::clone(&context.platform), request)
        .map(|action| Box::new(action) as Box<dyn ActionKind>)
}
