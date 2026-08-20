-- Unit U9's benchmark statistical methodology (SRS FR-BENCH-001..003,
-- TRS §34). One row per benchmark run: the baseline/post-change window
-- bounds and stability (coefficient of variation), the Mann-Whitney U
-- p-value, the resulting verdict (IMPROVED/REGRESSED/INCONCLUSIVE/
-- UNSTABLE, or BASELINE_UNSTABLE for a pre-flight abort where the action
-- was never applied), TRS §34's named confounders, and whether TRS §35's
-- rollback triggers fired.
CREATE TABLE benchmark_runs (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at                TEXT NOT NULL,
    action_id                 INTEGER REFERENCES actions (id),
    metric_name               TEXT NOT NULL,
    direction                 TEXT NOT NULL,
    baseline_window_start     TEXT NOT NULL,
    baseline_window_end       TEXT NOT NULL,
    baseline_cv               REAL,
    post_change_window_start  TEXT,
    post_change_window_end    TEXT,
    post_change_cv            REAL,
    p_value                   REAL,
    verdict                   TEXT NOT NULL,
    confounders_json          TEXT NOT NULL,
    rolled_back                INTEGER NOT NULL DEFAULT 0,
    rollback_action_id        INTEGER REFERENCES actions (id)
);

CREATE INDEX idx_benchmark_runs_action_id ON benchmark_runs (action_id);
CREATE INDEX idx_benchmark_runs_created_at ON benchmark_runs (created_at);
