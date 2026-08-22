# 0064 — DBMS Security Detection: Table Permission-Grant Detector

## Status

Accepted (unit U59).

## Context

FR-DBSEC-001(c), "permission changes," is the one DBMS security-event
sub-item that isn't a mechanical continuation of the pattern the last
five units established — ADR 0060/0061/0062/0063 each flagged it as
needing a real scope decision (which object types? what counts as "a
change"? how much baseline noise is acceptable?), not a wiring
exercise. Per explicit user direction, this unit narrows the scope
deliberately and documents the decision in full, the same "narrow,
then name the rest as deferred" approach ADR 0061 used for `rolsuper`
before ADR 0063 filled in the role-membership remainder.

## The scope decision (resolved empirically, against a real instance)

**Tables only** — not sequences, functions, schemas, or row-level
security policies. `information_schema.table_privileges` is the real
SQL-standard catalog for exactly this.

**A real noise problem, and its real fix.** Querying `information_
schema.table_privileges` unfiltered returned 1,678 rows on a nearly-
empty fresh instance — overwhelmingly `pg_catalog`/`information_
schema` system-catalog PUBLIC grants, real PostgreSQL bootstrap noise,
confirmed by direct query, not anything a human granted. Filtering
`table_schema NOT IN ('pg_catalog', 'information_schema')` removed
essentially all of it.

**A second real noise source, also confirmed empirically.** Even for a
genuinely new user table, PostgreSQL reports the table *owner's own*
full privilege set (`SELECT`/`INSERT`/`UPDATE`/`DELETE`/`TRUNCATE`/
`REFERENCES`/`TRIGGER` — 7 rows) as if it had been "granted," even
though nothing was explicitly granted — it's implicit ownership.
Joining against `pg_tables.tableowner` and filtering `grantee <>
tableowner` removed this too.

**Both filters together, verified directly**: a fresh table with no
explicit grants produces exactly zero rows; a real, explicit `GRANT
SELECT ON widgets TO alice;` produces exactly one.

## Decision

New `core::dbms::TablePrivilegeGrant { grantee, schema, table,
privilege_type }` (detection input, mirrors `RoleMembership`). New
`DbmsAdapter::table_privilege_grants`, backed by the filtered query
above.

New `core::repository::table_privilege_history::record_seen` (mirrors
`role_membership_history.rs` exactly, a 4-column composite key). New
table `known_table_privilege_grants` (migrations/V18).

New `core::security::detector::detect_table_privilege_granted`: same
`known_*` "have we ever seen this before" family as U57/U58 — fires
on a genuinely new tuple's first observation. No additional in-Rust
filtering — the SQL query already excludes the noise. `Severity::
Medium` (matching `detect_guc_change`/`detect_untrusted_device`'s own
tier — a real access change worth review, not automatically an active
escalation the way a superuser/role-membership grant is), category
`"PERMISSION_CHANGE"`.

`service/src/api/v1/dbms.rs`'s `gucs` handler gains a fourth
best-effort step, same resilience posture as the prior three.

**Verification approach, matching ADR 0061/0063's own precedent**:
neither of those units added a separate `core`-level smoke test
directly against the real `PostgresAdapter` query — the real SQL
correctness is proven end-to-end by the service-level HTTP test
instead, which can only produce a correct detection result if the
underlying SQL genuinely works. This unit follows the same precedent.

## Explicitly deferred (named, not silently dropped)

- Sequence-, function-, and schema-level privileges — a different
  catalog per object type, each its own scope decision.
- Column-level privileges (`information_schema.column_privileges`) —
  a separate, finer-grained catalog.
- Row-level security policies (`pg_policies`) — a fundamentally
  different mechanism than an ACL grant, not "permission changes" in
  the sense this unit covers.
- Revocation tracking — grants only, the same limitation ADR 0063
  already named for role-membership grants: a tuple, once known, stays
  known forever, so a real revoke-then-re-grant won't fire twice.
- **FR-DBSEC-001(a), repeated authentication failure — the one
  sub-item that remains fully open after this unit.** It needs
  server-log ingestion, a fundamentally different I/O class this
  codebase's SQL-view-only `core::dbms` design doesn't support today.

## Verification (real, not simulated)

- `core/src/security/detector.rs`'s own unit tests (10 cases total, 1
  new).
- `core/tests/security_detector_table_privilege_grant_test.rs` (3
  tests, mirrors ADR 0063's exact shape): repository durability across
  a fresh connection (including a different `privilege_type` for the
  same `(grantee, schema, table)` correctly reading as unknown); the
  pure detector's fire/no-fire rules; one real end-to-end proof
  producing a real `security_events`/`security_incidents` row.
- `service/tests/dbms_endpoints.rs`'s new
  `gucs_polling_a_real_table_grant_produces_a_real_security_incident_
  but_owner_privileges_do_not`: a real `CREATE TABLE`, polled with zero
  explicit grants first — proving the owner-implicit-grant filter for
  real, not just against a synthetic assertion — then a real `GRANT
  SELECT ON widgets TO alice;`, polled again, producing a real
  `MEDIUM`-severity incident naming `alice`/`widgets`/`SELECT`. Passed
  on the first real attempt and 3/3 reruns.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean (31/31 in `dbms_endpoints.rs`). `repository_migrations_up_
  down.rs`'s `EXPECTED_TABLES` updated. Both grep gates clean. Schema-
  drift clean (no `contracts` change). `cargo deny check`/`cargo
  audit` clean. No leaked PostgreSQL processes. One pre-existing,
  already-documented flaky test observed during a full run (`deadlock_
  history_reports_a_real_induced_deadlock`, a real timing-sensitive
  race from U39/ADR 0044, file untouched by this unit) — reran clean
  2/3 in isolation, confirmed pre-existing and unrelated.

## Consequences

- FR-DBSEC-001 now has real coverage for four of its five sub-items
  ((b) superuser grants and role-membership grants, (d) unusual client
  addresses, (e) configuration changes, and now (c) table permission
  grants within the scope named above). Only (a), auth-failure
  detection, remains fully open — a genuinely different architectural
  undertaking (log ingestion), not a same-pattern continuation.
- This unit's own scope (tables, non-owner, non-system-schema, grants
  only) is a deliberate first slice of "permission changes," not the
  whole of FR-DBSEC-001(c) — the deferred sub-scopes above are real
  future work, not silently assumed complete.
