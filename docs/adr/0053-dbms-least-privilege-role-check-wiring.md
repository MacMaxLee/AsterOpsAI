# 0053 — Service: DBMS Least-Privilege Role Check Wiring

## Status

Accepted (unit U48).

## Context

ADR 0025 (U20) named a real, deliberate gap and explicitly flagged it as
"not silently inherited": `service`'s real DB connection path builds a
pool directly (`dbms_config.rs` → `pool::build_pool` →
`PostgresAdapter::from_pool`), which skips the least-privilege check
`PostgresAdapter::connect` performs automatically
(`core::dbms::role_check::validate`). `role_check.rs` was already fully
built and core-tested (`core/tests/dbms_permission_denied_test.rs`,
`core/tests/security_detector_superuser_override_test.rs`) — the same
"fully built, never actually called from `service`" shape this
session's other wiring units (DBMS endpoints, `core::ai`) already
closed.

Confirmed by direct read before implementing:

- `role_check::validate` only ever rejects when `environment ==
  Environment::Production` **and** the role is a real superuser **and**
  no override is set — otherwise it's a harmless pass-through.
  `dbms_config.rs` hardcoded `environment: Environment::Development` and
  `allow_superuser_override: false`, so calling `validate` unchanged
  would never reject anything. The real fix needed both: actually
  calling `validate` somewhere in `service`'s connection path, *and*
  making `environment` genuinely configurable.
- `policy_config.rs`'s own doc comment already flags
  `dbms::connection_metadata::Environment` as structurally unrelated to
  `policy::risk::Environment` — a new, separate env var was needed
  (`$ASTEROPS_DB_ENVIRONMENT`), not a reuse of
  `$ASTEROPS_POLICY_ENVIRONMENT`.
- `main.rs`'s binary crate isn't reachable by `service/tests/*.rs`
  (integration tests target the `service` *library* crate) — the
  connect-and-validate logic had to move into `service/src/`, not stay
  inline in `main.rs`, to be genuinely testable.
- `TestPostgres`'s own default role (`pg.superuser`, `"postgres"`) is a
  real PostgreSQL superuser — this made a genuinely real, not simulated,
  end-to-end proof possible: build a real `TestPostgres`, classify its
  connection metadata as `Environment::Production` with
  `allow_superuser_override: false`, and confirm the wired connect path
  really refuses it.

## Decision

**`dbms_config.rs`** gained two new trailing parameters on
`resolve_db_connection_from` — `environment: Option<String>`,
`allow_superuser_override: Option<String>` — threaded into
`ConnectionMetadata` instead of the two previously-hardcoded values.
`resolve_db_environment_from` mirrors `policy_config.rs`'s own parse
style exactly: unset/unrecognized falls back to `Development`, the safe
default. `allow_superuser_override` parses `"true"`/`"1"` to `true`,
anything else (including unset) to `false` — refuse a superuser role in
Production by default rather than silently permit one. Two new env
vars: `$ASTEROPS_DB_ENVIRONMENT`, `$ASTEROPS_DB_ALLOW_SUPERUSER_OVERRIDE`.

**New `service/src/dbms_connect.rs`** — `connect_and_validate(metadata,
password, repository: Option<&RepositoryHandle>) -> Option<Arc<dyn
DbmsAdapter>>`. Builds the pool exactly as before, checks out a real
client, calls `role_check::validate`. Every failure path — pool build,
checkout, or a real `PermissionDenied` rejection — logs via
`tracing::error!` and returns `None`, the identical resilience
precedent `main.rs` already applied to a pool-build failure before this
unit: `service` keeps running without a DB adapter rather than crashing
over a DB connectivity/permission problem. A successful validation with
`override_used` records the real audit event
(`role_check::audit_event_for_override`) via
`repository::record_audit_event` when a repository is available — a
best-effort write; its own failure is logged (`tracing::warn!`), not
propagated, matching every other audit-event call site's resilience.
Otherwise wraps the pool in `PostgresAdapter::from_pool` exactly as
before.

**`main.rs`** now calls `dbms_connect::connect_and_validate(...)`
instead of its own inline `pool::build_pool` +
`PostgresAdapter::from_pool` block, passing `repository.as_ref()` (the
`Option<RepositoryHandle>` already in scope at that point in `serve()`).

**No change to `core::dbms::role_check`/`PostgresAdapter`** — pure
wiring, matching the unit's own FORBIDDEN scope. **No new
`CredentialStore` integration** — this unit keeps ADR 0025's own
deliberate "plain env var, no keyring" design; `connect_and_validate`
still calls `pool::build_pool` directly, not `PostgresAdapter::connect`
(which would require `CredentialStore`), and instead runs the same
`role_check::validate` call manually against the pool it already has.

## Verification (real, not simulated)

`service/tests/dbms_connect_test.rs` (3 new tests, real
`TestPostgres` PostgreSQL 17 instances, real superuser role, real
SQLite-backed `RepositoryHandle`):

- A connection classified `Environment::Production` with
  `allow_superuser_override: false` (unset) is genuinely refused —
  `connect_and_validate` returns `None`.
- The same connection with `allow_superuser_override: true` is let
  through (`Some`) **and** records a real
  `DBMS_SUPERUSER_OVERRIDE` audit row, read back via the existing
  `repository::latest_audit_event_type` helper — not asserted against a
  mock, against a real SQLite write.
- `Environment::Development` (today's real default, both fields unset)
  passes through unchanged with **no** audit event recorded — proves
  the unit didn't alter existing, un-configured behavior.

Also updated: `dbms_config.rs`'s own 6 pre-existing unit tests
(mechanical `None, None` trailing args, no behavior change) plus 10 new
unit tests covering `resolve_db_environment_from`/
`resolve_allow_superuser_override_from` parsing (staging/production/
unset-falls-back-to-Development, `"true"`/`"1"`/anything-else), mirroring
`policy_config.rs`'s own existing test style. 4 external integration
test call sites (`ai_endpoints.rs`, `correlation_endpoints.rs` ×2,
`dbms_endpoints.rs`) updated with the same mechanical `None, None`.

Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
clean. Both grep gates clean (no new SQL — `role_check`'s own query
lives in `core/src/dbms/adapters/postgresql/queries.rs`, already
exempt). Schema-drift clean (no wire-contract change — this is a
server-internal hardening fix, no new `contracts` type). `cargo deny
check`/`cargo audit` clean. No leaked PostgreSQL processes. One
pre-existing, already-documented flaky test observed during a full
`--test-threads=1` run (`core::tuning_plan_concurrency_test::
a_second_concurrent_plan_for_the_same_target_is_rejected_not_queued`,
a real timing-sensitive concurrency race, file untouched by this unit)
— reran clean 3/3 in isolation, confirmed pre-existing and unrelated.

## Consequences

- **This closes ADR 0025's own named gap.** `service`'s real DB
  connection path now runs the same least-privilege check
  `PostgresAdapter::connect` always performed — a superuser role
  against a `Production`-classified instance is refused by default,
  with an explicit, audited override path.
- Today's actual deployments are unaffected: `$ASTEROPS_DB_ENVIRONMENT`
  unset still means `Development`, the same permissive pass-through
  behavior as before this unit — this is additive hardening, not a
  behavior change for anyone not setting the new env var.
- `role_check::audit_event_for_override` — built since `core`'s own
  DBMS unit but never called from `service` until now — is a live code
  path for the first time.
