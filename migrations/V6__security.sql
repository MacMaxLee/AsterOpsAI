-- Schema for unit U9+'s security detection; no code writes to these tables
-- yet.
CREATE TABLE security_incidents (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    opened_at   TEXT NOT NULL,
    closed_at   TEXT,
    severity    TEXT NOT NULL,
    status      TEXT NOT NULL,
    summary     TEXT NOT NULL
);

CREATE TABLE security_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ts            TEXT NOT NULL,
    detector_id   TEXT NOT NULL,
    severity      TEXT NOT NULL,
    category      TEXT NOT NULL,
    summary       TEXT NOT NULL,
    evidence_json TEXT NOT NULL,
    incident_id   INTEGER REFERENCES security_incidents (id)
);

CREATE INDEX idx_security_events_ts ON security_events (ts);
CREATE INDEX idx_security_incidents_status ON security_incidents (status);
