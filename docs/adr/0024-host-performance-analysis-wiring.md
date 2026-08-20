# 0024 — Service + Console: Host Performance Analysis Wiring

## Status

Accepted (unit U19).

## Context

U5 shipped `core::analysis::classify_host`/`classify_db` — pure,
synchronous, fully unit-tested classification logic — but neither was
ever exposed through `service` or the console. ADR 0017 (the
correlation-engine unit) explicitly forward-referenced this exact gap:
"a future service/console-wiring unit should promote
`HostVerdict`/`DbHealthVerdict`/`CorrelationResult` together,
coherently, not this unit alone." This unit is that promotion, scoped to
the host side only.

`classify_db` and full `correlate()` (which needs both a host and a DB
verdict) are deliberately **not** wired here: `service` has no
`DbmsAdapter`/DB-connection concept anywhere — confirmed by grep before
writing the plan (zero references to `DbmsAdapter`/`dbms::` in
`rust_core/service/src/`). Wiring the DB side means inventing real
connection-management/config infrastructure from scratch, a materially
larger, separate unit. `core::ai` wiring is likewise out of scope — a
separate, larger candidate (live provider calls, its own contracts
types).

## Decision

**Six new `contracts` types, mapped field-for-field, not reinvented.**
`HostBottleneck` (10 variants), `HostDomain` (4), `Tier` (4), `Evidence`,
`DomainSignal`, `HostVerdict` mirror `core::analysis::{host,
thresholds, evidence}` exactly. `contracts` has no workspace-internal
dependency on `core`, so `service`'s handler does the mapping explicitly
(`to_wire_bottleneck`/`to_wire_domain`/`to_wire_tier`/`to_wire`) rather
than trying to share types across the boundary.

**A fixed 5-minute lookback window, no query-parameter override this
unit.** The sampler's normal interval is 1s
(`service::telemetry::sampler::NORMAL_INTERVAL`), so 5 minutes
comfortably clears `MIN_SAMPLES_FOR_CLASSIFICATION` (3) even under the
5s back-off interval — matching the "smallest real slice" discipline
every service-wiring unit in this project has kept.

**`200 OK` with a real verdict is the only response the repository-
present path can return; `503 Unavailable` is reserved for "the
repository layer never started."** `classify_host` already treats empty
history as a valid input (`HostBottleneck::Unknown` plus a
`sample_count` evidence entry), so the handler never turns that into an
error.

## Real discovery: "an empty repository" isn't reachable through the
normal wiring path

While writing `analysis_endpoints.rs`'s empty-repository test, the
first attempt (reusing the same `build_app` helper every other
integration test file in `service` uses, which passes the *same*
`Option<RepositoryHandle>` to both `AppState` and
`telemetry::sampler::spawn`) got `HostBottleneck::NONE` back, not the
expected `UNKNOWN`.

The real cause: `telemetry::sampler::spawn`'s Linux implementation calls
`sampler.tick()` and, if `repository` is `Some`, synchronously persists
that first real snapshot **before `spawn` ever returns** — not after
the first `interval` tick. Every service process that starts with a
repository configured has already written at least one real telemetry
row by the time any request can possibly reach it. "A repository that's
started but has zero snapshots yet" is not a real-world state this
service ever passes through — it's an artifact of test wiring that
happened to decouple the two, and the first version of this test
accidentally didn't decouple them.

Fixed by adding a second test helper,
`build_app_with_empty_repository`, that deliberately spawns the sampler
with `repository: None` (so it never persists) while still handing the
real, empty repository to `AppState` directly — reproducing the
genuinely-empty-history path the classifier itself needs to be tested
against, rather than asserting on a state the production binary would
never actually be in.

**Not a bug in `classify_host` or the sampler** — both behave exactly as
designed (eager first-sample persistence is a deliberate, reasonable
choice so `/cpu`, `/memory`, etc. never have to wait a full interval for
their first real reading). It's a real fact about this system's startup
behavior worth having on record, since it means "insufficient data"
verdicts from `/api/v1/analysis/host` will only ever appear briefly
after startup, in practice, not as a durable steady state on a working
install.

## Console

Same `PolledRepository`/`StreamProvider.autoDispose` read-only pattern
as U17's tuning history (no mutation this unit). The bottleneck value,
tier, and domain labels get a presentation-only string mapping in
`host_analysis_screen.dart` (`_bottleneckLabel`/`_domainLabel`/
`_tierLabel`) — same "icon/label picked from what the server already
sent, never re-derived" discipline ADR 0023 already established for
severity; `HostBottleneck.unknown` renders through the exact same path
as every other bottleneck, with its own honest label ("insufficient
data"), never silently hidden or reinterpreted as "no bottleneck."
Twelfth `NavigationRail` destination — inherits U17's scroll fix with no
changes needed, reconfirmed by a full 30/30 `flutter test` run.

## Consequences

- `GET /api/v1/analysis/host` and its console screen are the first real
  exposure of any `core::analysis` output outside `core`'s own test
  suite.
- DB-side classification (`classify_db`) and full `correlate()` stay
  entirely unbuilt in `service`/console — a future unit's job, and it
  will need to build real `DbmsAdapter`/connection-management
  infrastructure in `service` first, not just add another handler.
- `core::ai` wiring remains a wholly separate, unstarted candidate.
- A pre-existing, unrelated flaky test was observed during this unit's
  full-workspace verification: `core`'s
  `tuning_plan_concurrency_test::a_second_concurrent_plan_for_the_same_
  target_is_rejected_not_queued` (U10) failed once under full-workspace
  parallel load, then passed 5/5 in isolated reruns — a real race
  condition in that test's own timing assumptions, not touched or fixed
  by this unit; left flagged here rather than silently ignored.
