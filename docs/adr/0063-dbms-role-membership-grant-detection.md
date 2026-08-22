# 0063 — DBMS Security Detection: Role-Membership Grant Detector

## Status

Accepted (unit U58).

## Context

ADR 0061 (U56) narrowed FR-DBSEC-001(b), "newly granted superuser/role
privileges," to just a role's `rolsuper` flag, explicitly naming
general role-membership grants (`GRANT some_role TO other_role`, via
`pg_auth_members`) as "a real, separate, many-to-many data model...
not this narrow slice." This unit closes that deferred remainder —
distinct from sub-item (c), "permission changes" (object/table ACLs),
which stays fully deferred, same as sub-item (a), auth-failure
detection.

**Confirmed tractable, empirically, before implementing**: `pg_auth_
members` is a real, queryable catalog, joinable against `pg_roles`
twice for real role names; readable by an unprivileged role the same
way `pg_roles` was for U56. A real `CREATE ROLE alice LOGIN; CREATE
ROLE admins; GRANT admins TO alice;` genuinely adds a real, queryable
row. A fresh instance's own bootstrap already creates 3 real rows
(`pg_monitor` is a member of `pg_read_all_settings`/`pg_read_all_
stats`/`pg_stat_scan_tables`) — real, expected PostgreSQL behavior,
confirmed by direct query, not something a user did.

**The detector's shape matches `detect_untrusted_device`/U57's
`detect_unusual_client_address`, not the diff-based GUC/rolsuper
shape**: a role-membership grant is a discrete, atomic fact — once
granted, it's just "known" — not a value that changes for one fixed
key. So this fires on a genuinely new `(member, granted_role)` pair's
very first observation.

## Decision

New `core::dbms::RoleMembership { member, granted_role }` (internal
detection input, mirroring `RoleSuperuserFlag`). New `DbmsAdapter::
role_memberships`, backed by `SELECT m.rolname, g.rolname FROM pg_
auth_members am JOIN pg_roles m ON m.oid = am.member JOIN pg_roles g
ON g.oid = am.roleid`.

New `core::repository::role_membership_history::record_seen` (mirrors
`device_trust.rs`/U57's `client_address_history.rs` exactly, with a
composite `(member_role, granted_role)` key). New table `known_role_
memberships` (migrations/V17).

New `core::security::detector::detect_role_membership_granted`: fires
only for a genuinely new pair. **Noise filter, mirroring `detect_
untrusted_device`'s own `removable`-only restriction**: only considers
memberships where the *member* role's name doesn't start with `pg_` —
PostgreSQL's reserved prefix for every built-in system role. This
excludes the 3 bootstrap rows from firing as if a human had granted
something, without excluding real, human-driven grants. `Severity::
High` (matching `detect_role_superuser_granted`'s own tier — the same
class of privilege-escalation concern), category
`"PRIVILEGE_ESCALATION"`.

`service/src/api/v1/dbms.rs`'s `gucs` handler gains a third best-effort
step, same resilience posture as U55/U56/U57.

**A real, accepted, named limitation**: this unit tracks grants only,
never revocations — a `(member, granted_role)` pair, once recorded as
known, stays known forever, so a real revoke-then-re-grant of the
exact same pair won't fire a second time. Solving that would need a
genuine revocation-tracking design (does a "vanished" pair mean
revoked, or just not polled recently?) — a real, separate question,
not attempted here.

## Verification (real, not simulated)

- `core/src/security/detector.rs`'s own unit tests (9 cases total, 1
  new, including the `pg_*` filter case).
- `core/tests/security_detector_role_membership_grant_test.rs` (3
  tests, mirrors U57's exact shape): repository durability across a
  fresh connection (including a different `granted_role` for the same
  member correctly reading as unknown); the pure detector's fire/
  no-fire rules; one real end-to-end proof producing a real
  `security_events`/`security_incidents` row.
- `service/tests/dbms_endpoints.rs`'s new
  `gucs_polling_a_real_new_role_membership_grant_produces_a_real_
  security_incident`: a real `CREATE ROLE`/`GRANT ... TO` followed by
  a single `/gucs` poll (no baseline poll needed — this detector fires
  on first observation, unlike U55/U56's diff-based ones) produces a
  real `HIGH`-severity incident naming `alice`, and explicitly proves
  the real, pre-existing `pg_monitor` bootstrap memberships never
  fire. Passed on the first real attempt and 3/3 reruns.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean (30/30 in `dbms_endpoints.rs`). `repository_migrations_up_
  down.rs`'s `EXPECTED_TABLES` updated. Both grep gates clean. Schema-
  drift clean (no `contracts` change). `cargo deny check`/`cargo
  audit` clean. No leaked PostgreSQL processes.

## Consequences

- FR-DBSEC-001(b) is now fully closed: both the `rolsuper` flag (U56)
  and general role-membership grants (this unit) are detected.
- Four of five FR-DBSEC-001 sub-items are now closed. **Only two
  remain, both meaningfully larger than any slice shipped in this
  session's own FR-DBSEC-001 vein**: permission/ACL-change detection
  (needs a real design decision on grant granularity across every
  object type) and repeated-authentication-failure detection (needs
  server-log ingestion, a fundamentally different I/O class this
  codebase's SQL-view-only `core::dbms` design doesn't support today).
- This unit tracks grants only, not revocations — a real, named,
  accepted limitation, not silently assumed to be complete.
