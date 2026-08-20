# 0012 — Policy Engine: Schema Design, Single-Use Approvals, and the Test/Production Action-Surface Split

## Status

Accepted (unit U7).

## Context

U7 implements SRS §17-19 (FR-POL-001..006) and TRS §24-27: the typestate
chain, approval lifecycle, protected-resource registry, and nine-stage
executor pipeline every future mutating action must go through. Several
concrete design questions came up that neither SRS/TRS pins nor any earlier
unit resolved, each settled by reading the actual code/schema rather than
guessed, and two real bugs were found and fixed via genuine end-to-end
testing rather than assumed correct — recorded here so the reasoning and
the fixes survive past this session.

## Decision

**One row is one full action-lifecycle instance — no separate "approvals"
table.** `migrations/V4__actions.sql`'s `actions` table already had almost
the right shape (`status`, `approved_by`, `target_identity_json`,
`evidence_json`, ...) with nothing writing to it yet. `migrations/
V9__policy_approval_columns.sql` adds five `ALTER TABLE` columns
(`requested_by`, `parameters_json`, `parameters_hash`,
`resource_descriptor_json`, `approval_expires_at`) rather than a parallel
table: a row is created at proposal (`PENDING_APPROVAL`/`AUTO_ALLOWED`/
`DENIED`), transitions forward-only through `APPROVED` → `EXECUTING` →
`EXECUTED`/`FAILED`, and a rollback is a *new* row with `rollback_of` set
to the original — never a mutation of the original's own final state.

**Single-use is a consequence of one atomic compare-and-swap, not a second
mechanism.** `repository::policy::transition` runs `UPDATE actions SET
status = ?new WHERE id = ?id AND status = ?expected [AND approval_expires_at
> ?now]`. Every lifecycle step (`grant`, `authorize`/consume, record
result, start rollback) goes through this one function. Because the single
writer thread (ADR 0007) processes one command at a time, two concurrent
`authorize()` calls against the same row can never both see `affected ==
1` — the second always finds the status already moved to `EXECUTING` and
fails. This is what makes TRS §25's single-use/replay rejection real
without any extra locking primitive.

**Real bug found and fixed during testing: a parameter-count mismatch in
`transition`'s SQL.** The original implementation shared one `params![]`
call (7 values) across two SQL branches — the `check_not_expired = false`
branch's SQL only had 6 `?N` placeholders. rusqlite requires the bound
value count to match the statement's placeholder count, so this failed on
every non-expiry-checked transition (used by `execute`'s and `fail`'s own
result-recording transitions). Because `execute`'s failure path (`fail`) is
deliberately best-effort (`if let Err(write_err) = transition_action(...) {
tracing::warn!(...) }` — a secondary logging failure must never mask the
real error already being returned to the caller), this bug was **silently
swallowed**: `actions_executor_pipeline_test.rs`'s target-mismatch case
returned the correct `ActionError::TargetChanged` to the caller, but the
row's `status` column stayed at `EXECUTING` forever instead of moving to
`FAILED` — caught only by asserting the row's *persisted* status, not just
the function's return value. Fixed by giving each branch its own complete
`conn.execute(sql, params![...])` call with a matching placeholder/value
count, rather than one shared params array. A reminder that "best-effort
audit/bookkeeping write" and "the write is silently broken" look identical
from the caller's side unless a test checks the persisted state directly.

**Real bug found and fixed during testing: zombie processes read as
"alive."** The test-only `process_is_alive` helper (`core/tests/policy/
common/mod.rs`) originally checked only `Path::new("/proc/{pid}").exists()`
— true for a zombie ('Z' state) that hasn't been reaped by its parent yet,
not just a running process. `TestActionKind::apply()` kills a target
process it doesn't own a `Child` handle for (via a real `kill -TERM ...`
command, matching a real "kill an arbitrary process" action's own shape —
it wouldn't have spawned its target either), so the target legitimately
becomes a zombie in `SpawnedTarget`'s own process until the test process
calls `wait()` on it — `verify()`'s "is it actually dead" check was
reporting `true` (alive) for a process that had, in every real sense,
already terminated. Fixed by parsing the state character from `/proc/
[pid]/stat` and treating `'Z'` the same as "gone." A `Drop` impl was also
added to `SpawnedTarget` (matching `core/tests/dbms/common/mod.rs`'s
`TestPostgres` precedent) so a panicking assertion before explicit cleanup
can't leak the spawned process past the test.

**`RiskLevel` is five levels; SRS pins only the two end poles.** SRS
FR-POL-001 says "INFORMATIONAL through PROHIBITED" without naming what's
in between — `Informational`/`Low`/`Medium`/`High`/`Prohibited` is a
documented judgment call (ADR 0006/0010's own precedent for
under-specified thresholds). FR-POL-001's exhaustiveness requirement ("an
action type with no assigned risk cannot exist in a shipped build") is
enforced structurally, not by convention: `ActionKind::risk_level` is a
required trait method with no default implementation, so omitting it is a
compile error.

**`policy::Environment` is a new type, not a reuse of `dbms::
connection_metadata::Environment`.** The DBMS one is a field of
`ConnectionMetadata` — structurally a single PostgreSQL connection's own
setting, with one existing consumer (`role_check.rs`'s superuser rule).
FR-POL-002/004 need an environment concept governing host-level actions
that have nothing to do with a PG connection at all. Same three variants,
deliberately un-coupled — `core::policy` has no dependency on `core::dbms`.

**Protected-resource checking happens once, at execution time, not also at
proposal time.** TRS §26's fixed stage order lists it once, after approval
check and before capture-previous-state — checking only there (not
duplicated early, at `evaluate`) means the check reflects the resource's
*current* protection status, which could genuinely change between proposal
and execution (an operator could mark something protected in the interim);
an early check would only ever be as fresh as proposal time and add no
real safety over the one that already runs immediately before the mutating
stages.

**Zero real `ActionKind`/`TargetVerifier` implementations ship in
`core/src` this unit.** Three independent artifacts already say this:
`ProcessInfo.start_time_ticks`'s doc comment, `migrations/
V4__actions.sql`'s "no code writes to this table yet," and
`platform/*/exec.rs`'s "empty until unit U8." The only implementations
anywhere in this workspace live in `core/tests/policy/common/mod.rs` —
`TestActionKind` kills a real, disposable, test-spawned child process via
a real `kill -TERM` (the sanctioned `check-no-command-outside-exec.sh`
`core/tests/` exception, same one U4's PostgreSQL harness uses);
`ProcessTargetVerifier` re-reads a live process's real `/proc/[pid]/stat`
field 22. `actions_executor_pipeline_test.rs` proves the full nine-stage
pipeline against this real process, including a target-mismatch case that
aborts before `apply()` ever runs — not just a typecheck of the typestate
chain.

## Consequences

- `TargetVerifier` ended up living in `core::actions`, not `core::policy`
  as the original plan's file tree sketched — TRS §27's live
  re-verification is specifically an *execution-time* concern (right
  before `apply`/`rollback`), while `policy::approval::authorize`'s own
  "does this match what was proposed" check is a plain equality comparison
  against stored values, not a live-system probe. `TargetIdentity` (the
  data type) stays in `policy::target`; the verifier trait moved to
  `actions::target_verifier`.
- A future unit wiring up U8's real action types registers them into
  `policy::ActionTypeRegistry` and provides real `TargetVerifier`
  implementations (reading live process/DB-session state) — the framework
  this unit built doesn't need to change shape to accept them.
- `grant()` now writes its own `policy.granted` audit event (added after
  initially relying on the row's `approved_by` column alone, once
  `policy_audit_trail_test.rs` made clear FR-POL-005's "every policy
  decision" reasonably includes an operator's approval, not just
  proposal/denial outcomes) — a deliberate, tested addition, not an
  afterthought left undocumented.
