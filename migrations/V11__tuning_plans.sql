-- Unit U10's tuning engine (SRS FR-TUNE-001..003, TRS §36). One row per
-- tuning plan attempt against one target. The partial unique index is the
-- real, durable enforcement of FR-TUNE-002 ("at most one tuning plan may
-- be in flight per target at a time... a concurrent attempt is rejected,
-- not queued") — a second INSERT for a target that already has an
-- IN_FLIGHT row hits this constraint and fails, through the existing
-- single-writer thread (ADR 0007), no in-memory lock needed.
CREATE TABLE tuning_plans (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at            TEXT NOT NULL,
    target_identity_json  TEXT NOT NULL,
    target_start_time     INTEGER NOT NULL,
    profile               TEXT NOT NULL,
    mode                  TEXT NOT NULL,
    status                TEXT NOT NULL,
    completed_at          TEXT,
    candidates_json       TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_tuning_plans_one_in_flight_per_target
    ON tuning_plans (target_identity_json)
    WHERE status = 'IN_FLIGHT';

CREATE INDEX idx_tuning_plans_created_at ON tuning_plans (created_at);
