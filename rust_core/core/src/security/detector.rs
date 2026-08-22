//! Deterministic detector rules (SRS FR-SEC-001): pure functions over
//! caller-supplied evidence, no I/O, no AI dependency of any kind — same
//! boundary `core::analysis`'s own FR-PERF-005 note establishes for
//! performance classification. Each rule below is a documented id +
//! rationale pair; neither of the two shipped this unit needs a numeric
//! threshold (both are binary conditions), which is itself the documented
//! judgment call, not a threshold force-fitted where none exists.

use chrono::{DateTime, Utc};

use crate::dbms::log_tail::AuthFailureEvent;
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
pub const DETECTOR_GUC_CHANGED: &str = "dbms.guc_changed";
pub const DETECTOR_ROLE_SUPERUSER_GRANTED: &str = "dbms.role_superuser_granted";
pub const DETECTOR_UNUSUAL_CLIENT_ADDRESS: &str = "dbms.unusual_client_address";
pub const DETECTOR_ROLE_MEMBERSHIP_GRANTED: &str = "dbms.role_membership_granted";
pub const DETECTOR_TABLE_PRIVILEGE_GRANTED: &str = "dbms.table_privilege_granted";
pub const DETECTOR_AUTH_FAILURE: &str = "dbms.auth_failure";

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

/// Unit U55 (SRS FR-DBSEC-001(e)): fires only when `previous_setting`
/// is a real, previously-recorded value (`repository::guc_history::
/// record_guc_value`'s own `None` means "first-ever observation" —
/// seeding the baseline, not a change, the identical "empty history
/// isn't a real event" precedent `detect_untrusted_device` already
/// established) *and* it genuinely differs from `current_setting`. A
/// judgment call, not an SRS-pinned severity: `Medium`, the same
/// tier `detect_untrusted_device` uses — worth flagging for review,
/// not inherently as dangerous as a privilege escalation.
pub fn detect_guc_change(
    name: &str,
    previous_setting: Option<&str>,
    current_setting: &str,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    let previous_setting = previous_setting?;
    if previous_setting == current_setting {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_GUC_CHANGED,
        severity: Severity::Medium,
        category: "CONFIGURATION_CHANGE",
        summary: format!(
            "configuration setting '{name}' changed from '{previous_setting}' to \
             '{current_setting}'"
        ),
        evidence_json: serde_json::json!({
            "name": name,
            "previous_setting": previous_setting,
            "current_setting": current_setting,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: name.to_string(),
        },
        ts: now,
    })
}

/// Unit U56 (SRS FR-DBSEC-001(b), narrowed to the `rolsuper` flag —
/// see this module's own file-level context, not general role-
/// membership grants): fires only on a real, known `false -> true`
/// transition. Never on first observation (`previous_rolsuper: None`
/// — a role that's superuser on its very first poll, e.g. the
/// bootstrap `postgres` role, is not itself evidence of a *newly*
/// granted privilege, the same "empty history isn't a real event"
/// precedent every detector in this file shares). Never when already
/// `true` (no new grant happened). Never on a transition *to* `false`
/// — a revocation, the opposite of what SRS's "newly granted" wording
/// asks for.
pub fn detect_role_superuser_granted(
    rolname: &str,
    previous_rolsuper: Option<bool>,
    current_rolsuper: bool,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    if previous_rolsuper != Some(false) || !current_rolsuper {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_ROLE_SUPERUSER_GRANTED,
        severity: Severity::High,
        category: "PRIVILEGE_ESCALATION",
        summary: format!("role '{rolname}' was newly granted superuser privilege"),
        evidence_json: serde_json::json!({
            "rolname": rolname,
            "previous_rolsuper": previous_rolsuper,
            "current_rolsuper": current_rolsuper,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: rolname.to_string(),
        },
        ts: now,
    })
}

/// Unit U57 (SRS FR-DBSEC-001(d)): mirrors `detect_untrusted_device`'s
/// exact shape — "have we ever seen a connection from this address
/// before" is itself the interesting event, so this fires on a
/// genuinely new address's very first observation, unlike the
/// diff-based GUC/role-grant detectors above (which need a known
/// prior value to compare against). `already_known` comes from
/// `repository::client_address_history::record_seen` — real, durable
/// history, not a per-process-lifetime guess. Deterministic
/// first-time-seen only (FR-SEC-001's "no detector may depend on an
/// AI model" boundary) — no statistical/anomaly model.
pub fn detect_unusual_client_address(
    client_addr: &str,
    already_known: bool,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    if already_known {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_UNUSUAL_CLIENT_ADDRESS,
        severity: Severity::Medium,
        category: "UNUSUAL_CLIENT_ADDRESS",
        summary: format!("previously-unseen client address '{client_addr}' connected"),
        evidence_json: serde_json::json!({ "client_addr": client_addr }).to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: client_addr.to_string(),
        },
        ts: now,
    })
}

/// Unit U58 (SRS FR-DBSEC-001(b)'s deferred remainder — see this
/// module's own file-level context, general role-membership grants
/// via `pg_auth_members`, distinct from `detect_role_superuser_
/// granted`'s narrower `rolsuper` flag): mirrors `detect_untrusted_
/// device`'s exact shape — a role-membership grant is a discrete,
/// atomic fact, not a value that changes for a fixed key, so this
/// fires on a genuinely new `(member, granted_role)` pair's very
/// first observation. `already_known` comes from `repository::role_
/// membership_history::record_seen` — real, durable history.
///
/// Filters out every PostgreSQL built-in system role (the `pg_`
/// prefix is PostgreSQL's own reserved convention) as the *member* —
/// a fresh instance's own bootstrap already grants several such
/// memberships (e.g. `pg_monitor` in `pg_read_all_stats`), real,
/// expected PostgreSQL behavior, not something a human did. The same
/// "restrict to the actually-interesting subset" precedent `detect_
/// untrusted_device` already established via its own `removable`
/// filter.
pub fn detect_role_membership_granted(
    member: &str,
    granted_role: &str,
    already_known: bool,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    if already_known || member.starts_with("pg_") {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_ROLE_MEMBERSHIP_GRANTED,
        severity: Severity::High,
        category: "PRIVILEGE_ESCALATION",
        summary: format!("role '{member}' was newly granted membership in '{granted_role}'"),
        evidence_json: serde_json::json!({
            "member": member,
            "granted_role": granted_role,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: member.to_string(),
        },
        ts: now,
    })
}

/// Unit U59 (SRS FR-DBSEC-001(c), deliberately narrowed to table-level
/// DML privileges — see docs/adr/0064 for the full scope decision):
/// same `known_*` "have we ever seen this before" family as `detect_
/// role_membership_granted`. No additional in-Rust filtering is
/// needed here — the caller's own SQL query (`queries::table_
/// privilege_grants`) already excludes system schemas and the table
/// owner's own implicit grants, empirically confirmed to leave zero
/// baseline noise.
pub fn detect_table_privilege_granted(
    grantee: &str,
    schema: &str,
    table: &str,
    privilege_type: &str,
    already_known: bool,
    now: DateTime<Utc>,
) -> Option<DetectedEvent> {
    if already_known {
        return None;
    }
    Some(DetectedEvent {
        detector_id: DETECTOR_TABLE_PRIVILEGE_GRANTED,
        severity: Severity::Medium,
        category: "PERMISSION_CHANGE",
        summary: format!(
            "role '{grantee}' was newly granted {privilege_type} on '{schema}.{table}'"
        ),
        evidence_json: serde_json::json!({
            "grantee": grantee,
            "schema": schema,
            "table": table,
            "privilege_type": privilege_type,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: format!("{schema}.{table}"),
        },
        ts: now,
    })
}

/// Unit U60 (SRS FR-DBSEC-001(a), the last FR-DBSEC-001 sub-item — see
/// docs/adr/0065): unlike every detector above, this has **no
/// fire/no-fire branch and never returns `Option`** — every `event`
/// passed in already came from `dbms::log_tail::read_new_auth_
/// failures`, which only ever yields rows whose `sql_state_code`
/// already matched PostgreSQL's own auth-failure SQLSTATEs, and the
/// caller's persisted byte offset guarantees a given log line is
/// tailed (and therefore detected) exactly once — the same
/// "the input is inherently the event" shape as `detect_superuser_
/// override`'s own `Some`-only construction, just without even the
/// `override_used` guard. `Severity::High`, not `Medium`: a failed
/// auth attempt could be an attack in progress — a real judgment
/// call, since (per SRS's own "repeated authentication failure"
/// wording) severity doesn't yet scale with repetition, a named,
/// deferred refinement (docs/adr/0065).
///
/// A real bug caught by this unit's own service-level test, not
/// assumed correct from reading code alone: resource naming by
/// `connection_from` *alone* (mirroring `detect_unusual_client_
/// address`'s own choice) turned out wrong here — every local-socket
/// connection reports the identical `connection_from` value
/// (`"[local]"`), so two **genuinely unrelated** auth failures for
/// two different users from the same socket silently correlated into
/// one incident (FR-SEC-002's own "related events," stretched past
/// what "related" should mean), and since an incident's own summary
/// is set once, from whichever event opened it, the later user's
/// name never surfaced in it at all. Fixed to combine `user_name` and
/// `connection_from` into one composite resource key — the two
/// together are the real "who tried, from where" identity a repeat
/// offender's own *subsequent* failures should still correlate
/// against, while two different users no longer collide.
pub fn detect_auth_failure(event: &AuthFailureEvent, now: DateTime<Utc>) -> DetectedEvent {
    let user_display = event.user_name.as_deref().unwrap_or("<unknown>");
    let connection_display = event.connection_from.as_deref().unwrap_or("<unknown>");
    let resource_name = format!("{user_display}@{connection_display}");
    DetectedEvent {
        detector_id: DETECTOR_AUTH_FAILURE,
        severity: Severity::High,
        category: "AUTHENTICATION_FAILURE",
        summary: format!(
            "authentication failure for user '{user_display}': {}",
            event.message
        ),
        evidence_json: serde_json::json!({
            "user_name": event.user_name,
            "database_name": event.database_name,
            "connection_from": event.connection_from,
            "message": event.message,
            "log_time": event.log_time,
        })
        .to_string(),
        resource: ResourceDescriptor {
            kind: ResourceKind::Infrastructure,
            name: resource_name,
        },
        ts: now,
    }
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

    #[test]
    fn guc_change_fires_only_when_a_known_prior_value_genuinely_differs() {
        let now = Utc::now();
        assert!(
            detect_guc_change("autovacuum", Some("on"), "off", now).is_some(),
            "a real change from a known prior value must fire"
        );
        assert!(
            detect_guc_change("autovacuum", None, "on", now).is_none(),
            "the first-ever observation of a GUC must never fire — it seeds \
             the baseline, not a change"
        );
        assert!(
            detect_guc_change("autovacuum", Some("on"), "on", now).is_none(),
            "an unchanged value must never fire"
        );
    }

    #[test]
    fn role_superuser_granted_fires_only_on_a_real_known_false_to_true_transition() {
        let now = Utc::now();
        assert!(
            detect_role_superuser_granted("app_user", Some(false), true, now).is_some(),
            "a real grant from a known non-superuser role must fire"
        );
        assert!(
            detect_role_superuser_granted("postgres", None, true, now).is_none(),
            "a role that's already superuser on its first-ever poll must never fire"
        );
        assert!(
            detect_role_superuser_granted("app_user", Some(true), true, now).is_none(),
            "an already-superuser role firing again is not a new grant"
        );
        assert!(
            detect_role_superuser_granted("app_user", Some(true), false, now).is_none(),
            "a revocation (true -> false) must never fire — this detector is grants only"
        );
        assert!(
            detect_role_superuser_granted("app_user", Some(false), false, now).is_none(),
            "an unchanged non-superuser role must never fire"
        );
    }

    #[test]
    fn unusual_client_address_fires_only_for_a_genuinely_new_address() {
        let now = Utc::now();
        assert!(detect_unusual_client_address("127.0.0.1", false, now).is_some());
        assert!(
            detect_unusual_client_address("127.0.0.1", true, now).is_none(),
            "an already-known address never fires"
        );
    }

    #[test]
    fn role_membership_granted_fires_only_for_a_new_non_system_member() {
        let now = Utc::now();
        assert!(detect_role_membership_granted("alice", "admins", false, now).is_some());
        assert!(
            detect_role_membership_granted("alice", "admins", true, now).is_none(),
            "an already-known pair never fires"
        );
        assert!(
            detect_role_membership_granted("pg_monitor", "pg_read_all_stats", false, now).is_none(),
            "a built-in pg_* system role member never fires, even when new"
        );
    }

    #[test]
    fn table_privilege_granted_fires_only_for_a_new_tuple() {
        let now = Utc::now();
        assert!(
            detect_table_privilege_granted("alice", "public", "widgets", "SELECT", false, now)
                .is_some()
        );
        assert!(
            detect_table_privilege_granted("alice", "public", "widgets", "SELECT", true, now)
                .is_none(),
            "an already-known tuple never fires"
        );
    }

    #[test]
    fn auth_failure_always_produces_a_high_severity_event() {
        let now = Utc::now();
        let event = AuthFailureEvent {
            user_name: Some("pwuser".to_string()),
            database_name: Some("postgres".to_string()),
            connection_from: Some("127.0.0.1:46060".to_string()),
            message: "password authentication failed for user \"pwuser\"".to_string(),
            log_time: now,
        };
        let detected = detect_auth_failure(&event, now);
        assert_eq!(detected.severity, Severity::High);
        assert_eq!(detected.resource.name, "pwuser@127.0.0.1:46060");
        assert!(detected.summary.contains("pwuser"));
    }

    #[test]
    fn auth_failure_resource_combines_user_and_connection_so_different_users_never_collide() {
        let now = Utc::now();
        let same_address_different_users = [
            AuthFailureEvent {
                user_name: Some("parallels".to_string()),
                database_name: None,
                connection_from: Some("[local]".to_string()),
                message: "role \"parallels\" does not exist".to_string(),
                log_time: now,
            },
            AuthFailureEvent {
                user_name: Some("pwuser".to_string()),
                database_name: Some("postgres".to_string()),
                connection_from: Some("[local]".to_string()),
                message: "password authentication failed for user \"pwuser\"".to_string(),
                log_time: now,
            },
        ];
        let resources: Vec<String> = same_address_different_users
            .iter()
            .map(|event| detect_auth_failure(event, now).resource.name)
            .collect();
        assert_ne!(
            resources[0], resources[1],
            "two different users failing from the same connection_from must not \
             collide into the same resource (and therefore the same incident)"
        );
    }

    #[test]
    fn auth_failure_resource_degrades_gracefully_when_a_field_is_absent() {
        let now = Utc::now();
        let event = AuthFailureEvent {
            user_name: Some("pwuser".to_string()),
            database_name: None,
            connection_from: None,
            message: "password authentication failed for user \"pwuser\"".to_string(),
            log_time: now,
        };
        let detected = detect_auth_failure(&event, now);
        assert_eq!(detected.resource.name, "pwuser@<unknown>");
    }
}
