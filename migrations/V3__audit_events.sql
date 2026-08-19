-- Append-only, immutable (SRS FR-HIST-002) tamper-evident log. `id` is part
-- of row_hash's canonical encoding and is assigned explicitly by the writer
-- (not left to AUTOINCREMENT's own counter) so it's known before insert —
-- see core::repository::audit. Never targeted by size-ceiling eviction:
-- deleting any row invalidates every hash after it.
CREATE TABLE audit_events (
    id            INTEGER PRIMARY KEY,
    ts            TEXT NOT NULL,
    event_type    TEXT NOT NULL,
    actor         TEXT NOT NULL,
    summary       TEXT NOT NULL,
    detail_json   TEXT NOT NULL,
    prev_hash     TEXT NOT NULL,
    row_hash      TEXT NOT NULL UNIQUE
);

CREATE INDEX idx_audit_events_ts ON audit_events (ts);
CREATE INDEX idx_audit_events_type ON audit_events (event_type);
