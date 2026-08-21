//! Startup wiring for `core::actions`' real, shipped action types (unit
//! U22). Shared by `main.rs` (the real service) and this crate's own
//! integration tests, which need the identical registry to exercise
//! `grant`'s real authorize -> execute path — not a second, divergent
//! copy of the registration logic.

use ai_ops_core::policy::ActionTypeRegistry;

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
        registry.register(
            "host.set_process_priority",
            host::set_process_priority_entry(),
        );
        registry.register(
            "host.set_process_cpu_affinity",
            host::set_process_cpu_affinity_entry(),
        );
    }
    registry
}
