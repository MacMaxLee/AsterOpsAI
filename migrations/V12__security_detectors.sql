-- Unit U11's security detector framework (SRS FR-SEC-001..005, TRS §37).
-- security_events/security_incidents were created by V6 but never written
-- to; this migration finally gives them a resource-identity column
-- (needed for FR-SEC-003's "a flag on a resource" check) and adds the two
-- small tables this unit needs.

ALTER TABLE security_events ADD COLUMN resource_descriptor_json TEXT NOT NULL DEFAULT '{}';

-- Durable "have we ever seen this device before" tracking (FR-SEC-001's
-- untrusted-device detector). Replaces the service sampler's own
-- ephemeral in-process HashSet, which reset on every restart — a real,
-- previously-flagged gap this unit closes.
CREATE TABLE known_devices (
    identifier    TEXT PRIMARY KEY,
    first_seen_at TEXT NOT NULL
);

-- FR-SEC-005: false positives are manageable per-rule and per-resource.
-- resource_descriptor_json NULL means "suppress this detector_id
-- everywhere"; non-NULL suppresses it only for that exact resource.
CREATE TABLE security_suppressions (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at                TEXT NOT NULL,
    created_by                TEXT NOT NULL,
    detector_id               TEXT NOT NULL,
    resource_descriptor_json  TEXT,
    reason                    TEXT NOT NULL
);

CREATE INDEX idx_security_suppressions_detector ON security_suppressions (detector_id);
