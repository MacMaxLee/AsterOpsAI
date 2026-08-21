//! Startup wiring for `core::actions`' real, shipped action types (unit
//! U22), and the shared error-mapping/verifier logic every caller that
//! reaches into `core::policy`/`core::actions`/`core::tuning` needs (unit
//! U22's `policy.rs`, unit U23's `tuning.rs`). One real implementation,
//! every caller — not a second, divergent copy per handler module.

use ai_ops_core::actions::{ActionError, TargetVerifier};
use ai_ops_core::policy::{ActionTypeRegistry, PolicyError};
use ai_ops_core::tuning::TuningError;
use contracts::ApiError;

/// Every real, shipped `ActionKind` (unit U8) this service can execute —
/// registered under the exact `action_type` strings `core::tuning::
/// candidates.rs` already proposes tuning actions under. Linux-only:
/// `core::actions::host` has no Windows/macOS implementation yet
/// (`platform`'s own stub returns `CapabilityError::Unsupported` for the
/// underlying capability), so registering these elsewhere would only ever
/// fail at `check_capability()` — harmless, but pointless; an empty
/// registry gives the same real, honest `UnknownActionType` failure with
/// less indirection.
pub fn build_action_registry() -> ActionTypeRegistry {
    let mut registry = ActionTypeRegistry::new();
    #[cfg(target_os = "linux")]
    {
        use ai_ops_core::actions::host;
        use ai_ops_core::security::response;
        registry.register(
            "host.set_process_priority",
            host::set_process_priority_entry(),
        );
        registry.register(
            "host.set_process_cpu_affinity",
            host::set_process_cpu_affinity_entry(),
        );
        // Unit U26: fully built and core-tested since unit U11
        // (TRS §37 / SRS FR-SEC-004), but never previously registered —
        // real SIGSTOP/SIGCONT, RiskLevel::High, so `decide` requires
        // approval in every environment, not just Production.
        registry.register(
            "security.suspend_process",
            response::suspend_process_entry(),
        );
    }
    registry
}

/// The one real `TargetVerifier` (unit U8's `core::actions::host`,
/// Linux-only — `core::actions::host` has no Windows/macOS
/// implementation yet). On other platforms `AppState.action_registry` is
/// always empty (`build_action_registry`'s own Linux gate), so
/// `authorize()` always fails with a real `UnknownActionType` before a
/// verifier would ever be consulted — this stub exists purely so callers
/// compile cross-platform (CLAUDE.md's CI matrix), never to be genuinely
/// invoked.
#[cfg(not(target_os = "linux"))]
struct UnsupportedTargetVerifier;

#[cfg(not(target_os = "linux"))]
impl TargetVerifier for UnsupportedTargetVerifier {
    fn verify(&self, _expected: &ai_ops_core::policy::TargetIdentity) -> Result<(), ActionError> {
        Err(ActionError::CapabilityUnavailable(
            "host actions are not implemented on this platform yet".to_string(),
        ))
    }
}

#[cfg(target_os = "linux")]
pub fn target_verifier() -> impl TargetVerifier {
    ai_ops_core::actions::host::ProcessTargetVerifier
}

#[cfg(not(target_os = "linux"))]
pub fn target_verifier() -> impl TargetVerifier {
    UnsupportedTargetVerifier
}

pub fn policy_error_to_api(err: PolicyError) -> ApiError {
    match err {
        PolicyError::NotFound(_) => ApiError::NotFound,
        PolicyError::WrongStatus {
            id,
            expected,
            actual,
        } => ApiError::BadRequest(format!(
            "action {id} is not in the expected status: expected {expected}, actual {actual}"
        )),
        PolicyError::Expired(id) => {
            ApiError::BadRequest(format!("action {id}'s approval has expired"))
        }
        // The action type itself has no registered entry at all (unit
        // U22) — a real, server-wide capability gap (e.g. this platform
        // has no `core::actions::host` implementation yet), distinct
        // from `ActionError::CapabilityUnavailable` below, which fires
        // *after* a real entry was found, for a request-specific reason
        // (its target vanished, etc.) — that one is a 400, this is a 501.
        PolicyError::UnknownActionType(action_type) => {
            ApiError::Unsupported(format!("no registered action type: {action_type}"))
        }
        // Unit U26: `/actions/propose` is the first caller anywhere in
        // `service` to hand raw, client-supplied JSON `parameters`
        // straight to `core::policy::validate()` — every earlier caller
        // (tuning's own candidate-building) only ever validates
        // internally-constructed, pre-vetted parameters, so this arm was
        // unreachable (and absent) until now. A genuine client mistake,
        // not a server fault.
        PolicyError::InvalidParameters {
            action_type,
            reason,
        } => ApiError::BadRequest(format!(
            "invalid parameters for action type {action_type}: {reason}"
        )),
        // Unit U43 (ADR 0034/0048): the same target already has an
        // unresolved action of the same type — a real, client-
        // correctable condition (resolve or roll back the existing one
        // first), same category as `TuningError::PlanAlreadyInFlight`'s
        // own 400 (ADR 0028), not a server fault.
        PolicyError::ConflictingActionInFlight => ApiError::BadRequest(
            "a conflicting action is already in flight for this target".to_string(),
        ),
        other => {
            tracing::error!(error = %other, "unexpected policy error");
            ApiError::Internal
        }
    }
}

/// Unlike `policy_error_to_api`, every branch here reflects a real,
/// already-applied-or-attempted execution outcome (unit U22) — the row
/// has moved past `APPROVED` by the time any of these can happen, so
/// these are reported honestly as failures rather than folded into a
/// generic success (this project's consistent no-fabricated-success
/// discipline).
pub fn action_error_to_api(err: ActionError) -> ApiError {
    match err {
        ActionError::Policy(policy_err) => policy_error_to_api(policy_err),
        // A real discovery building unit U22: `check_capability()`
        // implementations (e.g. `SetProcessCpuAffinityAction`'s own)
        // directly probe the live target as part of the check itself —
        // so this fires for a request-specific reason (the target
        // vanished between grant and execute), not "this server can't do
        // this at all." `core::actions::ActionError` doesn't distinguish
        // the two cases in its own type, but by the time `execute()` is
        // running, the action type *was* found in the registry, so a 400
        // (this specific request can't be fulfilled) is the honest
        // mapping here, not a 501 (the server doesn't implement this).
        ActionError::CapabilityUnavailable(reason) => ApiError::BadRequest(reason),
        other => {
            tracing::warn!(error = %other, "action execution failed");
            ApiError::BadRequest(other.to_string())
        }
    }
}

/// Unit U23: `start_plan`'s own error surface. `PlanAlreadyInFlight`/
/// `UnsupportedTarget` are real, request-specific rejections (the
/// concurrency guard is a real partial `UNIQUE` index, migrations/V11;
/// `UnsupportedTarget` fires for a `DbSession` target, which the wire
/// contract never actually offers, but the mapping stays honest anyway).
/// `Policy`/`Action` delegate to the mappers above rather than
/// duplicating them.
pub fn tuning_error_to_api(err: TuningError) -> ApiError {
    match err {
        TuningError::PlanAlreadyInFlight => {
            ApiError::BadRequest("a tuning plan is already in flight for this target".to_string())
        }
        TuningError::UnsupportedTarget => {
            ApiError::BadRequest("tuning against a DbSession target is not supported".to_string())
        }
        TuningError::PlanNotFound(id) => {
            tracing::error!(
                plan_id = id,
                "tuning plan not found after we just created it"
            );
            ApiError::NotFound
        }
        TuningError::CapabilityUnavailable(reason) => ApiError::BadRequest(reason),
        TuningError::Policy(policy_err) => policy_error_to_api(policy_err),
        TuningError::Action(action_err) => action_error_to_api(action_err),
        other => {
            tracing::error!(error = %other, "unexpected tuning error");
            ApiError::Internal
        }
    }
}
