# 0016 — Security Detector Framework: Schema Gap, FR-SEC-003 Placement, and the Closed Response-Action Set

## Status

Accepted (unit U11).

## Context

`security_incidents`/`security_events` (migrations/V6) sat fully migrated
and completely unused since early in this project — the V6 migration's
own comment ("Schema for unit U9+'s security detection; no code writes to
these tables yet") had already slipped from U9 to U10 before this unit
finally wrote to them. SRS §20 (FR-SEC-001..005) and TRS §37 describe a
detector framework, severity + correlation, a policy override, a closed
response-action set, and per-rule/per-resource suppression. Several
concrete decisions came up that neither document pins.

## Decision

**`security_events` needed a real schema change, not a workaround.** Its
V6 columns had no resource-identity column at all — nothing to correlate
against a `ResourceDescriptor` for FR-SEC-003's "flag on a resource."
`migrations/V12__security_detectors.sql` adds
`resource_descriptor_json TEXT NOT NULL DEFAULT '{}'` to it.
`security_incidents` itself needed no change — correlation is a real join
through its linked events (`repository::security::find_open_incident`),
not a duplicated column.

**Two detectors ship this unit, both grounded in already-real signals —
no new telemetry needed.** `dbms.superuser_override_used` consumes
`dbms::role_check::validate`'s already-computed `ValidationOutcome`
(unit U4) — today that only produces a soft `audit_events` row; this unit
adds a second, severity-carrying, correlatable record on top, without
changing `role_check::validate`'s own behavior or its existing audit
write. `host.untrusted_device_attached` closes a real, previously-flagged
gap: `contracts::telemetry::device::DeviceInfo.trusted`'s own doc comment
said "already known" tracking was an ephemeral, per-process `HashSet` in
the service sampler, "until unit U2 adds real persistence" — U2 shipped
and never did. `repository::device_trust::record_seen` (`known_devices`,
migrations/V12) makes "first-seen" durable across restarts, proven by
`security_detector_untrusted_device_test.rs` reopening a fresh
`RepositoryHandle` against the same on-disk file and confirming
"known" survives. The detector only fires for `removable` devices — a
fixed internal disk reporting "new" on the very first-ever service run
(empty `known_devices` table) would otherwise flood a security event for
every disk on first install; that's a real, motivated filter, not an
arbitrary trim.

**FR-SEC-003's override check lives in `actions::executor::execute`, not
`policy::evaluate`** — the same "freshest possible check, at
execute-time" reasoning ADR 0012 already established for the
protected-resource stage, added as a new stage immediately after it.
This needed no new field on `ActionTypeEntry` (unlike U10's `reversible`,
which had to live there because that gate ran *before* construction): the
security-flag check runs where a real `Arc<dyn ActionKind>` already
exists, so a single defaulted trait method,
`ActionKind::is_security_response() -> bool` (default `false`), is
enough. The check is `if !action.is_security_response() { ... }` —
a security response action is never blocked by its own resource's flag,
which is the whole point of FR-SEC-003's "overrides any
*performance-motivated* action," proven end to end by
`security_flag_overrides_performance_action_test.rs`: a real OPEN
incident against a spawned process blocks `host.set_process_priority`
with `ActionError::SecurityFlagged`, while `security.suspend_process`
against the exact same flagged target executes normally.

**FR-SEC-004's closed-enum guarantee has no literal `ActionType` enum to
extend** — TRS §37 names one ("the `ActionType` enum structurally cannot
contain a firewall/AV-disabling variant") that was never built anywhere
in the codebase; grep confirmed zero hits before this unit.
`security::response::SecurityResponseAction` is a real, structural
guarantee instead: an enum with an exhaustive `action_type()` match, and
it is the *only* function anywhere that maps a security-response type to
its registration entry — nothing else can register a security-response-
flagged action type through this path, and adding a firewall/AV-disabling
variant would require deliberately touching this exact enum and match.
This is honestly narrower than "the whole `ActionTypeRegistry` is
closed" (it's still a `HashMap<String, ActionTypeEntry>` under the hood,
same as every other action type) — a wholesale redesign of action-type
dispatch as an enum instead of a string-keyed registry is real,
cross-cutting work this unit doesn't take on.

**`BLOCK_DEVICE` (TRS §37's other named example) is deliberately not
shipped this unit — flagged, not silently dropped.** `TargetIdentity` has
exactly two variants (`Process`, `DbSession`); devices have no natural
"start time" analog for TRS §27's re-verify step, so a `Device` variant
would need real design work of its own (what does re-verification even
mean for a device — does a device path get reused the way a PID does?),
not a hasty bolt-on. TRS's own "e.g." wording doesn't mandate exactly two
response actions at first ship. One real, fully-tested response action
(`SuspendProcess`) is more honest than a second one that's only
half-real.

**Real discovery from testing: `SIGSTOP`/`SIGCONT` delivery is
asynchronous, unlike U8's `setpriority`/`sched_setaffinity`.** The first
version of `SuspendProcessAction::apply()` just called
`platform::suspend_process` and returned — `verify()` immediately after
found the process not actually stopped yet, a genuine timing race, not a
flaky test: `kill()` returning `0` only means the signal was *sent*, not
that the kernel has scheduled the target to act on it yet. Fixed by
polling `is_process_stopped`/`!is_process_stopped` for up to 1s (50 ×
20ms) inside `apply()`/`rollback()` themselves — the same real-completion-
polling precedent `core/tests/policy/common/mod.rs`'s own
`TestActionKind::apply()` already uses after its own `kill -TERM`, now
applied in production code for the same underlying reason (signal
delivery isn't synchronous). `verify()` itself stays a simple one-shot
check, consistent with U8's own verify() implementations, because
`apply()` doesn't return `Ok` until the real transition has already been
observed.

**Suppression gates *before* an incident is even opened, not just an
already-open one.** `repository::security::record_event_checked` checks
`is_suppressed` first; a match writes nothing at all — no event, no
incident, no noise. The suppression's own creation still goes through the
existing `audit_events` chain (`security::suppress` calls
`record_audit_event` in the same call), not a second parallel audit
mechanism — FR-SEC-005's "recorded audit trail" is the one this project
already has.

## Consequences

- A future unit adding `BLOCK_DEVICE` (or any device-targeted action)
  must first design `TargetIdentity::Device` and its own re-verification
  semantics — real, separate work, not something this ADR pre-decides.
- `security::response::SecurityResponseAction` currently has exactly one
  variant; adding a second means touching this exact enum's exhaustive
  match, which is the guarantee working as intended, not friction to
  route around.
- `resource_is_flagged`'s query has no notion of severity threshold —
  *any* `OPEN` incident against a resource blocks *every*
  non-security-response action against it, regardless of the incident's
  own severity. A future unit wanting e.g. "only `HIGH`+ incidents block"
  would need to widen this query, not work around it elsewhere.
- `security_incidents.status` is `OPEN`/`CLOSED` only — this unit never
  writes `CLOSED` (nothing yet decides when an incident is resolved).
  A future unit owning incident resolution/closure is real, separate
  work; until then, every incident this unit opens stays `OPEN` and
  keeps its resource flagged indefinitely, which is the conservative,
  safe default, not an oversight.
