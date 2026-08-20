# 0017 — Correlation Engine: Cause Mapping, the Two-Shaped Confidence Formula, and Deferred Contracts Promotion

## Status

Accepted (unit U12).

## Context

`core::analysis`'s own module doc (U5) forward-referenced this unit
directly: "Cross-layer correlation (SRS §31/TRS §20) is a distinct,
later unit that consumes this module's output types as input." SRS §31
(FR-CORR-001..003) and TRS §20 describe ranked root-cause hypotheses
over host + DB evidence, each with a confidence value from "a documented
formula" and an explicit ruled-out list with reasons. The literal
next-numbered unit (U12) was reserved throughout the codebase for real
Windows/macOS platform support, but that work can't be verified for real
in this Linux-only sandbox — even this project's own CI only `cargo
check`s those targets, never runs them. The user chose Correlation
Engine instead this session, deferring literal U12 until real hardware/
CI runners are available.

## Decision

**FR-CORR-002's nine causes are a real, documented mapping onto
`DbHealthCategory`/`HostBottleneck`, not a 1:1 correspondence.**
`DbHealthCategory` has 11 variants, `HostBottleneck` has 10; FR-CORR-002
names nine causes "at minimum." The mapping:

| Cause | Source |
|---|---|
| `DbLocks` | `LockWaits` + `Deadlocks` + `LongTransactions` |
| `DbConfiguration` | `TempFileUsage` + `BloatProxies` (the two existing checks most directly attributable to tunable GUC settings — no dedicated "GUC misconfiguration" detector exists) |
| `ConnectionExhaustion` | `ConnectionSaturation` |
| `SlowSql` | `Latency` + `SlowQueries` |
| `HostCpu`/`HostMemory`/`StorageLatency`/`Network` | the matching `HostDomain`'s own `DomainSignal` directly |
| `ClientSideApplication` | not directly evidenced — see below |

`RollbackRatio`, `ReplicationLag`, and `Availability` map to none of the
nine — "at minimum" doesn't require every `DbHealthCategory` to have a
cause, and this is stated here rather than silently dropped. A future
unit widening the cause list (or DBMS-side detectors) can claim them.

**The confidence formula genuinely needs two different shapes for its
two evidence sources — a deliberate difference, not an inconsistency.**
`analysis::db`'s own module doc already states DB evidence is "a
single-poll snapshot, not a window." So a DB-side cause's confidence is
the average severity of its mapped checks
(`confidence::db_cause_confidence`): `Ok`=0.0, `Warning`=0.5,
`Critical`=1.0, `Unavailable` excluded from both sides of the average
(0.0 if every mapped check is `Unavailable` — this is also how a
DB-unmonitored host is handled for free: `analysis::db::
unavailable_verdict`'s output drives every DB-side cause to confidence
0.0 with no `Option<DbHealthVerdict>` branch anywhere in `correlate`).
Host evidence is a real sustained window (`DomainSignal.sample_count`/
`crossed_count`) — a host-side cause's confidence is that domain's own
`crossed_count / sample_count` fraction (`confidence::
host_cause_confidence`), the same fraction `DomainSignal::crossed()`
already computes internally, just exposed continuously instead of as a
boolean.

**`ClientSideApplication` is computed from the other eight's results,
never its own signal — there isn't one.** `confidence = m * (1.0 - m)`,
where `m` is the strongest of the other eight causes' confidences. This
peaks at a moderate, unresolved signal (`m` around `0.5`) and is exactly
`0.0` at either extreme: nothing wrong anywhere (`m = 0.0`, nothing to
explain), or one cause fully and confidently explains it (`m = 1.0`, no
room left for "something we can't see"). `correlation_test.rs`'s
`a_moderate_unresolved_db_signal_makes_client_side_application_a_live_
hypothesis_too` proves this directly: a single `Warning`-level DB check
(confidence `0.5`) makes both `SlowSql` (`0.5`) and
`ClientSideApplication` (`0.25`) rank together, real, ordered uncertainty
rather than a single forced answer.

**`RULE_OUT_THRESHOLD = 0.1`** (same numeric-judgment-call style ADR
0010/0014/0015 already use) splits every computed cause into `ranked`
(above, sorted descending) or `ruled_out` (at or below, with a reason
string built from its own mapped evidence — "no DB evidence available"
when every mapped check was genuinely `Unavailable`, "DB checks show no
sustained problem (average severity N.NN)" when checks were available
but healthy, "no host evidence available"/"no sustained signal (crossed
fraction N.NN)" for the host-side equivalent).

**`CorrelationResult` stays `core`-internal — FR-CORR-003's "contract
types" is aspirational for this unit, not something to force now.**
FR-CORR-003 says correlation output "uses the same contract types the
console and the AI explanation layer consume." But this unit's own
inputs, `HostVerdict`/`DbHealthVerdict` (U5), are still entirely
`core`-internal — nothing in `contracts` represents any analysis output
yet, and `core::ai::bundle::EvidenceBundle` (U6, the AI layer's own
evidence shape) is core-internal too. Promoting `CorrelationResult` alone
to `contracts` while its own inputs aren't would be an inconsistent,
orphaned partial schema addition. A future service/console-wiring unit
(the same "no wiring yet" precedent every unit since U4 has kept) is
what should promote `HostVerdict`/`DbHealthVerdict`/`CorrelationResult`
together, coherently, not this unit alone.

**`correlate` consumes `analysis`'s output types directly, never raw
telemetry or a `RepositoryHandle`.** No new signal source, no I/O, no AI
dependency — matching `analysis`'s own FR-PERF-005 boundary and
`security::detector`'s own FR-SEC-001 boundary precedent exactly. Tests
construct `HostVerdict`/`DbHealthVerdict` fixtures directly (every field
is `pub`) rather than routing through `classify_host`/`classify_db`,
testing `correlate`'s own logic in isolation from threshold
classification, which `analysis_host_classification_test.rs`/
`analysis_db_health_*_test.rs` already cover on their own.

## Consequences

- A future unit adding a tenth-plus cause, or claiming
  `RollbackRatio`/`ReplicationLag`/`Availability`, extends
  `DB_CAUSE_MAP`/`HOST_CAUSE_MAP` and `RootCause`'s own variant list —
  `ClientSideApplication`'s formula needs no change, since it's computed
  from whatever the "other" causes report, not a fixed count.
  `RootCause::EVIDENCED` (used to size `correlate`'s candidate buffer and
  in one test's ruled-out-count assertion) must grow with it.
- A future service/console-wiring unit is what decides how
  `HostVerdict`/`DbHealthVerdict`/`CorrelationResult` become wire types
  together — this unit deliberately doesn't pre-guess that shape.
- `host_metric_name`'s string-matching against `HostVerdict.evidence`'s
  flat `Vec<Evidence>` is a real, if slightly fragile, coupling to
  `analysis::host::classify_host`'s own metric-name strings; a future
  change to those strings must update `correlate.rs`'s copy too — flagged
  here rather than discovered as a silent test gap.
