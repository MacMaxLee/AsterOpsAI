-- One row per (bucket). avg/min/max are computed only over source rows
-- whose corresponding *_state = 'SUPPORTED'; *_supported_count/*_total_count
-- let a reader see how much of a bucket is trustworthy signal vs.
-- gap/reset/unavailable — a 100%-unsupported bucket yields NULL avg/min/max
-- and supported_count = 0, never a fabricated number (FR-CAP-001's ethos,
-- applied to rollups).
CREATE TABLE telemetry_rollup_hourly (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_start                TEXT NOT NULL,
    bucket_end                  TEXT NOT NULL,

    cpu_util_avg REAL, cpu_util_min REAL, cpu_util_max REAL,
    cpu_util_supported_count INTEGER NOT NULL, cpu_util_total_count INTEGER NOT NULL,

    mem_used_avg REAL, mem_used_min REAL, mem_used_max REAL,
    mem_used_supported_count INTEGER NOT NULL, mem_used_total_count INTEGER NOT NULL,

    storage_read_bps_avg REAL, storage_write_bps_avg REAL,
    net_rx_bps_avg REAL, net_tx_bps_avg REAL,

    UNIQUE (bucket_start)
);
CREATE INDEX idx_rollup_hourly_bucket ON telemetry_rollup_hourly (bucket_start);

-- Aggregated from telemetry_rollup_hourly (avg-of-avgs, min-of-mins,
-- max-of-maxes), not a raw re-scan — raw for that day may already be purged
-- by the time the daily rollup runs. See core::repository::retention.
CREATE TABLE telemetry_rollup_daily (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_start                TEXT NOT NULL,
    bucket_end                  TEXT NOT NULL,

    cpu_util_avg REAL, cpu_util_min REAL, cpu_util_max REAL,
    cpu_util_supported_count INTEGER NOT NULL, cpu_util_total_count INTEGER NOT NULL,

    mem_used_avg REAL, mem_used_min REAL, mem_used_max REAL,
    mem_used_supported_count INTEGER NOT NULL, mem_used_total_count INTEGER NOT NULL,

    storage_read_bps_avg REAL, storage_write_bps_avg REAL,
    net_rx_bps_avg REAL, net_tx_bps_avg REAL,

    UNIQUE (bucket_start)
);
CREATE INDEX idx_rollup_daily_bucket ON telemetry_rollup_daily (bucket_start);
