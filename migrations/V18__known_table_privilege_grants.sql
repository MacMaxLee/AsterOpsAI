-- Unit U59's table permission-grant detector (SRS FR-DBSEC-001(c),
-- deliberately narrowed to table-level DML privileges only -- see
-- docs/adr/0064 for the full scope decision and empirical noise
-- findings behind it).
-- Durable "have we ever seen this exact (grantee, schema, table,
-- privilege_type) tuple before" tracking -- mirrors known_role_
-- memberships (V17) exactly, just with a wider composite key.
CREATE TABLE known_table_privilege_grants (
    grantee       TEXT NOT NULL,
    table_schema  TEXT NOT NULL,
    table_name    TEXT NOT NULL,
    privilege_type TEXT NOT NULL,
    first_seen_at TEXT NOT NULL,
    PRIMARY KEY (grantee, table_schema, table_name, privilege_type)
);
