//! Deterministic detector rules (SRS FR-SEC-001): pure functions over
//! caller-supplied evidence, no I/O, no AI dependency of any kind — same
//! boundary `core::analysis`'s own FR-PERF-005 note establishes for
//! performance classification. Each rule below is a documented id +
//! rationale pair; neither of the two shipped this unit needs a numeric
//! threshold (both are binary conditions), which is itself the documented
//! judgment call, not a threshold force-fitted where none exists.

use chrono::{DateTime, Utc};

use crate::dbms::role_check::ValidationOutcome;
use crate::policy::{ResourceDescriptor, ResourceKind};

use super::severity::Severity;

/// What a detector produces before persistence decides whether it's
/// suppressed and which incident it correlates into (`core::security::
/// incident::record_event`).
#[derive(Debug, Clone)]
pub struct DetectedEvent {
    pub detector_id: &'static str,
    pub severity: Severity,
    pub category: &'static str,
    pub summary: String,
    pub evidence_json: String,
    pub resource: ResourceDescriptor,
    pub ts: DateTime<Utc>,
}

pub const DETECTOR_SUPERUSER_OVERRIDE: &str = "dbms.superuser_override_used";
pub const DETECTOR_UNTRUSTED_DEVICE: &str = "host.untrusted_device_attached";

/// Rationale: a superuser role connecting to a Production-classified
/// database instance is already refused outright unless the connection's
/// own `allow_superuser_override` was explicitly set
/// (`dbms::role_check::validate`, unit U4) — but "was explicitly allowed
/// through anyway" is itself security-relevant and worth a severity-
/// carrying, correlatable record, not just the soft `audit_events` row
/// `role_check::audit_event_for_override` already writes. Fires only when
/// the override was actually used — a superuser connection outside
/// Production, or one that never needed an override, is not this rule's
/// concern.
pub fn detect_superuser_override(
    outcome: &ValidationOutcome,
    connection_name: &str,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    if !outcome.override_used {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_SUPERUSER_OVERRIDE,
        severity: Severity::High,
        category: "PRIVILEGE_ESCALATION",
        summary: format!(
            "connection '{connection_name}' used a superuser role against a \
             PRODUCTION-classified instance via an explicit override"
        ),
        evidence_json: serde_json::json!({
            "connection_name": connection_name,
            "rolsuper": outcome.rolsuper,
            "override_used": outcome.override_used,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: connection_name.to_string(),
        },
        ts: now,
    })
}

/// Rationale: an unrecognized *removable* device is the classic
/// security-relevant device event (unauthorized USB/removable storage);
/// a fixed internal disk reporting "new" only ever means the service's
/// own device history is empty (e.g. first-ever run), not a real event,
/// so this rule only ever fires for `removable` devices. `already_known`
/// comes from `repository::device_trust::record_seen` — real, durable
/// history, not a per-process-lifetime guess.
pub fn detect_untrusted_device(
    identifier: &str,
    name: &str,
    removable: bool,
    already_known: bool,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    if already_known || !removable {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_UNTRUSTED_DEVICE,
        severity: Severity::Medium,
        category: "UNTRUSTED_DEVICE",
        summary: format!("previously-unseen removable device '{name}' attached"),
        evidence_json: serde_json::json!({
            "identifier": identifier,
            "name": name,
            "removable": removable,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Device,
            name: identifier.to_string(),
        },
        ts: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superuser_override_fires_only_when_override_was_used() {
        let now = Utc::now();
        assert!(detect_superuser_override(
            &ValidationOutcome {
                rolsuper: true,
                override_used: true,
            },
            "prod-primary",
            now,
        )
        .is_some());

        assert!(detect_superuser_override(
            &ValidationOutcome {
                rolsuper: true,
                override_used: false,
            },
            "prod-primary",
            now,
        )
        .is_none());

        assert!(detect_superuser_override(
            &ValidationOutcome {
                rolsuper: false,
                override_used: false,
            },
            "prod-primary",
            now,
        )
        .is_none());
    }

    #[test]
    fn untrusted_device_fires_only_for_new_removable_devices() {
        let now = Utc::now();
        assert!(detect_untrusted_device("block:sdb", "sdb", true, false, now).is_some());
        assert!(
            detect_untrusted_device("block:sdb", "sdb", true, true, now).is_none(),
            "already-known devices never fire"
        );
        assert!(
            detect_untrusted_device("block:sda", "sda", false, false, now).is_none(),
            "non-removable devices never fire, even when new"
        );
    }
}
