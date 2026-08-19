-- TRS §12 calls for `dbms_*` tables "namespaced ... future". SRS §26-34
-- describes 9 broad DBMS monitoring areas (sessions, query perf, locks,
-- storage, replication, config, security) whose exact fields aren't
-- knowable until unit U4's DbmsAdapter trait and real PostgreSQL
-- introspection queries exist. Rather than speculatively design 9
-- area-specific tables now, this migration adds ONE generic, namespaced,
-- forward-compatible table satisfying "tables per TRS §12" without
-- overcommitting to unknown schema (documented deviation, see ADR 0007).
-- Unit U4+ adds real dbms_sessions/dbms_locks/etc. tables via its own
-- migration once the adapter's method set (TRS §33) is implemented.
CREATE TABLE dbms_metrics (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ts            TEXT NOT NULL,
    instance_id   TEXT NOT NULL,
    family        TEXT NOT NULL,
    metric_name   TEXT NOT NULL,
    value         REAL,
    state         TEXT NOT NULL DEFAULT 'UNAVAILABLE',
    detail_json   TEXT
);

CREATE INDEX idx_dbms_metrics_instance_family_ts ON dbms_metrics (instance_id, family, ts);
