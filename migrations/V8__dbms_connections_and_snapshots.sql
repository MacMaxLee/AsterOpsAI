-- Real schema for unit U4's PostgreSQL monitoring, superseding V7's
-- generic dbms_metrics placeholder now that the adapter's real method set
-- (TRS §33) and return shapes are known. dbms_metrics was never written to
-- or read from by any shipped code (only asserted to exist by the
-- migration-list test) — safe to drop outright rather than leave a dead,
-- confusingly generic table sitting alongside a properly-typed one.
DROP TABLE dbms_metrics;

-- Connection metadata only — deliberately no password column. TRS §18 /
-- SRS NFR-PRIV-002: the password lives only in the OS credential store,
-- looked up at connect time via password_ref. This isn't written to by any
-- code yet either (SCOPE for this unit is the collection/adapter layer,
-- not the config-management surface a later unit adds) — the table exists
-- now because core::dbms::ConnectionMetadata's shape is already fixed and
-- this is where it's meant to persist.
CREATE TABLE dbms_connections (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    name                      TEXT NOT NULL UNIQUE,
    host                      TEXT NOT NULL,
    port                      INTEGER NOT NULL,
    database                  TEXT NOT NULL,
    username                  TEXT NOT NULL,
    tls_mode                  TEXT NOT NULL,
    environment               TEXT NOT NULL,
    password_ref              TEXT NOT NULL,
    allow_superuser_override  INTEGER NOT NULL DEFAULT 0,
    capture_raw_sql           INTEGER NOT NULL DEFAULT 0,
    created_at                TEXT NOT NULL
);

-- One row per poll of one connection — mirrors telemetry_snapshots'
-- shape exactly (V1__init_pragmas_and_core_tables.sql): the single
-- per-instance scalar (version/uptime) gets real columns; every
-- variable-cardinality result (sessions, query stats, locks, table/index
-- stats, replication standbys, GUCs, long transactions,
-- idle-in-transaction sessions) is JSON text, for the same reason U2's
-- telemetry design already established (rollups/analysis only need the
-- top-line scalars; per-row detail doesn't need 12 separate normalized
-- tables to be queryable later). No code writes to this yet — the live
-- poll loop that would is later-unit work (see the U4 plan's SCOPE note);
-- the schema exists now because the adapter's real return shapes are
-- already fixed, matching V4/V5/V6's own "schema now, writer later"
-- precedent.
CREATE TABLE dbms_snapshots (
    id                              INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id                   INTEGER NOT NULL REFERENCES dbms_connections (id),
    ts                               TEXT NOT NULL,

    version_string                   TEXT,
    server_version_num               INTEGER,
    started_at                       TEXT,

    databases_json                   TEXT,
    sessions_json                    TEXT,
    query_stats_state                TEXT NOT NULL DEFAULT 'UNAVAILABLE',
    query_stats_json                 TEXT,
    locks_json                       TEXT,
    table_stats_json                 TEXT,
    index_stats_json                 TEXT,
    replication_json                 TEXT,
    gucs_json                        TEXT,
    temp_files                       INTEGER,
    temp_bytes                       INTEGER,
    deadlocks                        INTEGER,
    long_transactions_json           TEXT,
    idle_in_transaction_json         TEXT
);

CREATE INDEX idx_dbms_snapshots_connection_ts ON dbms_snapshots (connection_id, ts);
