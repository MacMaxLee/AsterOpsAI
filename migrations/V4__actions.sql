-- Schema for units U6-U8's action pipeline; no code writes to this table
-- yet. target_start_time matches the /proc/[pid]/stat field-22 semantics
-- already used by unit U1's ProcessInfo.start_time_ticks, per requirement 4.
CREATE TABLE actions (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at           TEXT NOT NULL,
    action_type          TEXT NOT NULL,
    target_identity_json TEXT NOT NULL,
    target_start_time    INTEGER NOT NULL,
    risk_classification  TEXT NOT NULL,
    status               TEXT NOT NULL,
    approved_by          TEXT,
    executed_at          TEXT,
    rollback_of          INTEGER REFERENCES actions (id),
    evidence_json        TEXT NOT NULL,
    result_json          TEXT
);

CREATE INDEX idx_actions_status ON actions (status);
CREATE INDEX idx_actions_target_start_time ON actions (target_start_time);
