# 0079 — Revocation Tracking for Role-Membership and Table-Privilege Grants

## Status

Accepted (unit U74).

## Context

ADR 0063 (role-membership grants, FR-DBSEC-001b) and ADR 0064 (table
permission grants, FR-DBSEC-001c) both explicitly named the same gap
rather than silently accepting it: their "have we ever seen this tuple
before" tracking only ever inserts. A tuple, once known, stays known
forever — so a real `REVOKE` followed by a genuine later `GRANT` of the
exact same pair/tuple never fires again, because it still reads as
"already known." ADR 0064 called this out directly: "a tuple, once
known, stays known forever, so a real revoke-then-re-grant won't fire
twice." This is a real security-detection blind spot: an admin
revoking, then later re-granting, a suspicious permission (accidentally
or as part of an attack pattern) would go undetected the second time.

Confirmed directly, not assumed from the ADR text: `role_membership_
history::record_seen`/`table_privilege_history::record_seen` (both
in `rust_core/core/src/repository/`) only ever check-then-insert; grep
confirmed neither module had any `DELETE` statement.

## Decision

**Reconcile-before-record, once per sweep tick.** `dbms_security_
sweep::check_role_membership_grants`/`check_table_privilege_grants`
now call a new `repository::reconcile_known_role_memberships`/
`reconcile_known_table_privilege_grants` *before* their existing
per-tuple `record_*_seen` loop, passing the complete, freshly-polled
current set for this tick (not an incremental delta). Each reconcile
function deletes any row from its `known_*` table whose key isn't in
the current set — i.e. anything revoked since the last sweep — so a
later genuine re-grant of the same tuple reads as unknown again and
fires.

This mirrors the exact mark-and-sweep shape already used for retention
(`repository::retention::run_sweep`), just scoped to two small
composite-key tables instead of telemetry rows. No new migration is
needed — `known_role_memberships` (V17) and `known_table_privilege_
grants` (V18) already have the right composite primary keys; only a
`DELETE` was missing.

Implementation is plain, non-dynamic SQL (a `SELECT` of all known
tuples, an in-Rust set-difference against `current`, then one
parameterized `DELETE` per stale tuple) rather than a dynamic `WHERE
... NOT IN (...)` clause — the known-tuple counts here are small
(dozens, not thousands), and this avoids building SQL with a
variable-length parameter list.

## Verification (real, not simulated)

- `core/tests/security_detector_role_membership_grant_test.rs` (2 new
  tests): a revoked pair is forgotten and a genuine re-grant no longer
  reads as "already known"; a still-present pair survives reconcile
  untouched.
- `core/tests/security_detector_table_privilege_grant_test.rs` (2 new
  tests): the same two proofs, mirrored exactly for the 4-tuple key.
- `service/tests/dbms_endpoints.rs` (2 new, real end-to-end tests
  against a genuine PostgreSQL instance): `GRANT`, sweep (1 event) →
  `REVOKE`, sweep (still 1 event — a bare revocation must never fire) →
  re-`GRANT`, sweep (2 events — the fix's entire point). Asserted via a
  direct `security_events` count for the detector, not the `/security/
  incidents` list — a second event against the same still-`OPEN`
  incident correlates into it rather than opening a visibly distinct
  incident row, so the incidents API alone can't distinguish this case.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean (`service`'s own `dbms_endpoints.rs` suite: 35/35 passing).
  Both grep gates and the AI-reachability gate pass (unaffected).
- `docs/traceability-matrix.md` regenerated: FR-DBSEC-001 remains `OK`.

## Consequences

- FR-DBSEC-001(b)/(c) no longer have a standing "revoke-then-re-grant
  never fires twice" blind spot — ADR 0063/0064's own named gap is
  closed, not just documented.
- The reconcile step adds one extra read query and, in the common case
  of zero revocations since the last sweep, zero writes per tick — a
  negligible addition to `SWEEP_INTERVAL`'s existing 30s budget (ADR
  0078).
- The same mark-and-sweep shape is now available as a precedent for
  any future "durable have-we-seen-this-before" tracker that needs the
  same revocation-awareness (e.g. a future generalization of
  `client_address_history`/`device_trust`, should either ever need it).
