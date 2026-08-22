-- Unit U58's role-membership-grant detector (SRS FR-DBSEC-001(b)'s
-- own deferred remainder, per ADR 0061: general role-membership
-- grants via pg_auth_members, not just the rolsuper flag U56 covers).
-- Durable "have we ever seen this exact (member, granted_role) pair
-- before" tracking — mirrors known_devices (V12)/known_client_
-- addresses (V16) exactly, just with a composite key.
CREATE TABLE known_role_memberships (
    member_role   TEXT NOT NULL,
    granted_role  TEXT NOT NULL,
    first_seen_at TEXT NOT NULL,
    PRIMARY KEY (member_role, granted_role)
);
