-- Schema for unit U5's performance analysis/correlation engine; no code
-- writes to this table yet. score_version is a frozen weight-set id, never
-- mutated in place (SRS FR-PERF-004), per requirement 4.
CREATE TABLE performance_analysis (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ts            TEXT NOT NULL,
    subject       TEXT NOT NULL,
    score         REAL NOT NULL,
    score_version TEXT NOT NULL,
    verdict       TEXT NOT NULL,
    evidence_json TEXT NOT NULL
);

CREATE INDEX idx_perf_analysis_ts ON performance_analysis (ts);
CREATE INDEX idx_perf_analysis_subject_version ON performance_analysis (subject, score_version);
