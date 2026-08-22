-- Unit U60's authentication-failure detector (SRS FR-DBSEC-001(a)):
-- persisted byte-offset tailing state for a real, locally-readable
-- PostgreSQL CSV log file -- one row per log file path, so log
-- rotation (a new file name) starts a fresh offset rather than
-- reusing a stale one from a previous, now-inactive file.
CREATE TABLE log_tail_offsets (
    log_file_path TEXT PRIMARY KEY,
    byte_offset   INTEGER NOT NULL,
    updated_at    TEXT NOT NULL
);
