# 0061 — DBMS Security Detection: Superuser-Grant Detector

## Status

Accepted (unit U56).

## Context

ADR 0060 (U55) closed FR-DBSEC-001(e) — configuration-change detection
— and explicitly named sub-item (b), "newly granted superuser/role
privileges," as a natural second slice once that diff-based pattern
was proven. This unit closes that slice.

**Scoped narrowly**: SRS's wording bundles "superuser" and "role
privileges" loosely. This unit detects only a role's `rolsuper` flag
flipping `false -> true` — the single clearest, most security-critical
instance of "newly granted privilege" — not general role-membership
grants (`GRANT some_role TO other_role`, a real, separate,
many-to-many data model via `pg_auth_members`), and not permission
changes (table/object ACLs).

**Confirmed tractable, empirically, before implementing** (a real
throwaway PostgreSQL instance, not assumed):
- `pg_roles` is genuinely readable by an ordinary, unprivileged,
  non-superuser role with zero special grants — confirmed by directly
  connecting as a fresh `CREATE ROLE readonly_test LOGIN` role.
- Every built-in `pg_*` system role reports `rolsuper = f`; only the
  bootstrap `postgres` role starts `t` — no system-role noise.
- A real `ALTER ROLE app_user SUPERUSER;` genuinely and immediately
  flips `pg_roles.rolsuper` from `f` to `t`.
- Sub-item (d), unusual client addresses — also considered for this
  slot — was rejected: `pg_stat_activity.client_addr` is `NULL` for
  every Unix-socket connection (confirmed directly), so testing it for
  real would need `TestPostgres` to accept TCP connections, a
  genuinely larger, separate harness change this unit doesn't build.
  Role-superuser-grant detection needed none of that.

**No pre-existing query**: unlike GUCs, `core::dbms` had no "list every
role + its superuser flag" query before this unit — only `current_
user_is_superuser` (the connection's own role). This unit adds a
genuinely new `DbmsAdapter::role_superuser_flags` trait method.

**No perfect endpoint fit**: `GET /api/v1/dbms/gucs`'s handler — U55's
own "slow-changing DB-wide security/config posture" poll point — is
the least-contrived existing, already-polled home for this second
check, named explicitly here as a pragmatic choice, not a perfect
semantic one.

## Decision

New `core::dbms::RoleSuperuserFlag { rolname, rolsuper }` (internal
detection input, not a `contracts` type — no HTTP endpoint exposes raw
role data). New `DbmsAdapter::role_superuser_flags`, implemented via
`SELECT rolname, rolsuper FROM pg_roles ORDER BY rolname`.

New `core::repository::role_history::record_role_superuser_flag`
(mirrors `guc_history.rs` exactly): returns the previous flag, `None`
on first observation. New table `known_role_superuser_flags`
(migrations/V15).

New `core::security::detector::detect_role_superuser_granted`: fires
only on a real, known `false -> true` transition. Never on first
observation (a role that's superuser on its very first poll — e.g.
the bootstrap `postgres` role — is not itself evidence of a *newly*
granted privilege). Never when already `true`. Never on a transition
*to* `false` — a revocation, the opposite of "newly granted."
`Severity::High` (a role gaining superuser is more severe than a
config value changing — matching `detect_superuser_override`'s own
High tier for the adjacent concern), category `"PRIVILEGE_ESCALATION"`.

`service/src/api/v1/dbms.rs`'s `gucs` handler gains a second
best-effort step alongside U55's GUC-change check, same resilience
posture: logged, never blocks the real GUC response.

## A real discovery while writing the service-level test

The first version of the HTTP test created the role and granted it
superuser both *between* the two `/gucs` polls. It never fired: the
role's very first observation (on the second poll) already showed
`rolsuper = true`, which the detector's own first-observation rule
correctly treats as a baseline seed, not a transition. Fixed by
creating the role *before* the baseline poll, then granting superuser
between the two polls — the role's own tracked history has to start
before the baseline, exactly like the GUC test's own baseline
establishes for `autovacuum`. A real, caught test-design bug, not a
product issue — the same class of discovery this session's tests have
repeatedly surfaced and fixed rather than silently working around.

## Verification (real, not simulated)

- `core/src/security/detector.rs`'s own unit tests (5 cases).
- `core/tests/security_detector_role_superuser_grant_test.rs` (3
  tests, mirrors `security_detector_guc_change_test.rs`'s exact
  shape): repository durability across a fresh connection; the pure
  detector's fire/no-fire rules; one real end-to-end proof producing a
  real `security_events`/`security_incidents` row.
- `service/tests/dbms_endpoints.rs`'s new
  `gucs_polling_a_real_new_superuser_grant_produces_a_real_security_
  incident`: a real `CREATE ROLE`/`ALTER ROLE ... SUPERUSER` between
  two polls of the real `/gucs` endpoint produces a real, queryable
  `HIGH`-severity incident over `GET /api/v1/security/incidents`.
  Passed on the first corrected attempt and 3/3 reruns.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean (28/28 in `dbms_endpoints.rs`). `repository_migrations_up_
  down.rs`'s `EXPECTED_TABLES` updated. Both grep gates clean (new SQL
  lives in already-allowed `core::dbms::adapters`/`core::repository`
  locations). Schema-drift clean (no `contracts` change). `cargo deny
  check`/`cargo audit` clean. No leaked PostgreSQL processes. One
  pre-existing, already-documented flaky test observed during a full
  run (`deadlock_history_reports_a_real_induced_deadlock`, a real
  50ms-timing-sensitive race from U39/ADR 0044, file untouched by this
  unit) — reproduced intermittently even in isolation, confirmed
  pre-existing and unrelated.

## Consequences

- SRS FR-DBSEC-001(b) is now real and live for its narrowest, clearest
  case: a role gaining superuser privilege is detected and correlated
  into a real security incident, the next time `/api/v1/dbms/gucs` is
  polled.
- General role-membership-grant detection (`pg_auth_members`),
  permission-change (ACL) detection, unusual-client-address detection
  (confirmed to need a real new `TestPostgres` TCP capability), and
  auth-failure detection all remain real, separate, deferred future
  slices — FR-DBSEC-001 is not fully closed by this unit either.
