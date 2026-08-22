-- Unit U55's configuration-change detector (SRS FR-DBSEC-001(e)).
-- Durable "what was this GUC's setting the last time we polled it"
-- tracking — mirrors known_devices (V12) exactly: a first-ever
-- observation seeds the baseline, not a change.
CREATE TABLE known_guc_values (
    name       TEXT PRIMARY KEY,
    setting    TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
