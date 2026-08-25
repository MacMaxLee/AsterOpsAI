//! `default_capability_registry` (SRS FR-CAP-001, FR-CAP-002): the single
//! source of truth for what this build can currently report, consumed
//! identically by the server and by generated documentation. Real
//! platform-conditional behavior — `#[cfg(target_os = "linux")]` on the
//! test itself, mirroring the registry's own `cfg!(target_os = "linux")`
//! branch, since the two claims (Linux families Supported, the rest
//! Unavailable with a reason) are only meaningful together on Linux.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use contracts::CapabilityFamily;

#[test]
#[cfg(target_os = "linux")]
fn linux_implemented_families_are_supported_the_rest_are_unavailable_with_a_reason() {
    let registry = contracts::default_capability_registry();

    let implemented = [
        CapabilityFamily::Cpu,
        CapabilityFamily::Memory,
        CapabilityFamily::Storage,
        CapabilityFamily::Network,
        CapabilityFamily::Process,
        CapabilityFamily::Device,
    ];
    for family in implemented {
        assert_eq!(
            registry.get(&family),
            Some(&Capability::Supported),
            "{family:?} should be Supported on Linux"
        );
    }

    let not_yet_implemented = [
        CapabilityFamily::DbmsPostgresql,
        CapabilityFamily::Security,
        CapabilityFamily::Actions,
        CapabilityFamily::SelfMetrics,
    ];
    for family in not_yet_implemented {
        match registry.get(&family) {
            Some(Capability::Unavailable { reason }) => assert!(!reason.is_empty()),
            other => panic!("expected {family:?} to be Unavailable with a reason, got {other:?}"),
        }
    }
}

/// SRS FR-CAP-002: every family in the enum has exactly one entry —
/// nothing silently missing from the registry, nothing duplicated.
#[test]
fn every_capability_family_has_exactly_one_registry_entry() {
    let registry = contracts::default_capability_registry();
    let all_families = [
        CapabilityFamily::Cpu,
        CapabilityFamily::Memory,
        CapabilityFamily::Storage,
        CapabilityFamily::Network,
        CapabilityFamily::Process,
        CapabilityFamily::Device,
        CapabilityFamily::DbmsPostgresql,
        CapabilityFamily::Security,
        CapabilityFamily::Actions,
        CapabilityFamily::SelfMetrics,
    ];
    assert_eq!(registry.len(), all_families.len());
    for family in all_families {
        assert!(
            registry.contains_key(&family),
            "missing entry for {family:?}"
        );
    }
}
