-- Unit U56's superuser-grant detector (SRS FR-DBSEC-001(b), narrowed
-- to rolsuper flag flips, not general role-membership grants).
-- Durable "what was this role's rolsuper flag the last time we polled
-- it" tracking — mirrors known_guc_values (V14) exactly.
CREATE TABLE known_role_superuser_flags (
    rolname    TEXT PRIMARY KEY,
    rolsuper   INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);
