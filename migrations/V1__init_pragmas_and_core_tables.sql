-- Pragmas are applied at connection-open time in core::repository::connection
-- instead of here: refinery wraps each migration in a transaction, and
-- SQLite rejects changing `synchronous` (and effectively `journal_mode`)
-- inside one ("Safety level may not be changed inside a transaction").

-- One row per persisted sampler tick. Scalar metrics get real columns (cheap
-- AVG/MIN/MAX for rollups); variable-cardinality nested detail (per-core
-- array, volumes, interfaces) is stored as JSON text, since rollups only
-- aggregate the top-line scalars. Per-process/per-device detail is
-- deliberately not persisted here (unbounded / not metrically useful
-- historically) — see core::repository::telemetry_store.
CREATE TABLE telemetry_snapshots (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts                          TEXT NOT NULL,

    cpu_aggregate_util_pct      REAL,
    cpu_aggregate_util_state    TEXT NOT NULL DEFAULT 'UNAVAILABLE',
    cpu_load_avg_1m             REAL,
    cpu_pressure                TEXT NOT NULL,
    cpu_per_core_json           TEXT,

    mem_total_bytes             INTEGER,
    mem_used_bytes               INTEGER,
    mem_used_bytes_state         TEXT NOT NULL DEFAULT 'UNAVAILABLE',
    mem_available_bytes          INTEGER,
    mem_swap_used_bytes          INTEGER,
    mem_pressure                 TEXT NOT NULL,

    storage_read_bytes_ps        REAL,
    storage_write_bytes_ps        REAL,
    storage_volumes_json          TEXT,

    net_rx_bytes_ps                REAL,
    net_tx_bytes_ps                REAL,
    net_interfaces_json             TEXT,

    process_total_count            INTEGER,
    device_count                    INTEGER,

    containerized                   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_telemetry_snapshots_ts ON telemetry_snapshots (ts);
