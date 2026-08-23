# 0077 — Extract `gucs`'s Security/Config Sweep Into Its Own Function

## Status

Accepted (unit U71).

## Context

With the traceability backlog and the AI-explains-correlation feature
both closed, this unit went looking for other already-named, still-open
follow-ups in older ADRs — the same "check whether a later unit already
closed this before assuming it's still open" discipline this session
established with ADR 0069/0072/0073. A scan of 9 pre-session ADRs
(0038, 0042, 0044, 0046, 0052, 0055, 0056, 0063, 0065) found several
named gaps that turned out to already be closed by intervening units,
confirmed by direct code inspection rather than trusted from the ADR
text alone:

- ADR 0046's `long_transactions`/`idle_in_transaction_sessions`
  populated-path gap — already closed: `PostgresAdapter::
  with_activity_thresholds` (unit U53) exists, and
  `long_transactions_reports_a_real_long_running_transaction`/
  `idle_in_transaction_sessions_reports_a_real_idle_session` both
  already use it.
- ADR 0056's streaming-replication standby-populated gap — already
  closed: `dbms_replica_test.rs::replication_status_reports_a_real_standby`
  already runs a real primary + `pg_basebackup -R` standby.
- ADR 0063's "permission/ACL-change detection" remaining sub-item —
  already closed by ADR 0064.

**One genuinely still open**: ADR 0065's own note that `gucs`'s handler
had grown, one unit at a time (U55-U60), into a de facto "DB-wide
security/config sweep" doing meaningfully more than its name suggests,
naming extraction into a dedicated function as real, deferred work.
Confirmed directly: the handler still inlined all five `check_*` calls
verbatim, unchanged since U60.

## Decision

Extracted the five-call body into a new private
`run_security_config_sweep(repo, adapter, gucs)`, called once from
`gucs`. Pure extraction — same five checks
(`check_guc_changes`/`check_role_superuser_grants`/
`check_role_membership_grants`/`check_table_privilege_grants`/
`check_auth_failures`), same order, same inputs, no behavior change.
`gucs`'s own doc comment is trimmed to explain *why* this lives here
(no dedicated poll/scheduler infrastructure exists in `service` yet)
without also carrying the now-resolved "should probably extract this"
call to action.

**Deliberately not attempted**: building actual dedicated poll/
scheduler infrastructure so this sweep doesn't need to piggyback on
`gucs` at all. That's a real, separate, larger architectural question
(when should it run? on what interval? does it need its own error/
retry posture independent of the GUC read it currently rides along
with?) — this unit only does the narrow thing ADR 0065 explicitly
named, not that larger one it didn't.

## Verification (real, not simulated)

- All 32 tests in `service/tests/dbms_endpoints.rs` pass unchanged,
  including every `gucs_polling_a_real_*_produces_a_real_security_incident`
  test from units U55-U60 — proving the extraction preserved behavior
  exactly, not just compiled.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean.
- `docs/traceability-matrix.md` regenerated: no change, as expected for
  a pure code reorganization that adds no new test or FR-ID reference.

## Consequences

- `gucs.rs`'s own doc comment no longer describes unresolved design
  debt — a future reader sees an accurate, current description, not a
  standing TODO that's actually already been done partway (extracted,
  even if the larger "dedicated poll point" question remains open).
- The larger question — whether these five checks eventually deserve
  real, independent scheduling instead of riding along with `gucs` —
  remains open, named here rather than implied closed by this unit's
  narrower fix.
