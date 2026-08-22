-- Unit U57's unusual-client-address detector (SRS FR-DBSEC-001(d)).
-- Durable "have we ever seen a connection from this client address
-- before" tracking — mirrors known_devices (V12) exactly.
CREATE TABLE known_client_addresses (
    client_addr   TEXT PRIMARY KEY,
    first_seen_at TEXT NOT NULL
);
