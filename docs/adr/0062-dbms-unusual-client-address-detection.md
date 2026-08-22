# 0062 — DBMS Security Detection: Unusual Client Address Detector

## Status

Accepted (unit U57).

## Context

ADR 0060 (U55)/ADR 0061 (U56) closed FR-DBSEC-001(e) and (b), both
naming sub-item (d), "connections from unusual client addresses," as
real and directly precedented (mirrors `known_devices`/`detect_
untrusted_device` exactly) but deferred: `pg_stat_activity.client_addr`
is `NULL` for every Unix-socket connection, confirmed empirically, so
testing it for real needed `TestPostgres` to accept TCP connections —
a capability that didn't exist yet. This unit builds it and closes the
slice.

**The most minimal of the three FR-DBSEC-001 slices so far**:
`core::dbms::SessionInfo` already carries `client_addr: Option<String>`,
already populated by `list_sessions()`, already wired to the real `GET
/api/v1/dbms/sessions` endpoint since U31. No new `DbmsAdapter` method,
no new query, no new trait method was needed — unlike GUCs (reused an
existing query) and role-grants (needed a genuinely new one), this
slice was pure repository+detector+wiring work on data that already
flowed end-to-end.

**The detector's shape matches `detect_untrusted_device`, not the
diff-based GUC/role-grant shape**: "have we ever seen this specific
client address before" is itself the interesting event — a genuinely
new address fires on its very *first* observation, unlike a GUC or a
role's `rolsuper` flag, where the first poll is just an initial state,
not evidence of anything.

**Confirmed tractable, empirically, before implementing**: a plain
`-c listen_addresses='127.0.0.1'` override (via U51's already-existing
`start_with_extra_options`, no new `TestPostgres` capability) opens a
real TCP listener; `initdb --auth=trust`'s own default `pg_hba.conf`
already trust-authenticates `127.0.0.1`/`::1`. A real TCP connection
genuinely produces a real, non-null `client_addr` row.

## Decision

New `core::repository::client_address_history::record_seen` (mirrors
`device_trust.rs` exactly): returns whether already known. New table
`known_client_addresses` (migrations/V16).

New `core::security::detector::detect_unusual_client_address`: mirrors
`detect_untrusted_device` exactly — fires only when not already known.
`Severity::Medium`, category `"UNUSUAL_CLIENT_ADDRESS"`. Deterministic
first-time-seen only — no statistical/anomaly model, matching
FR-SEC-001's "no detector may depend on an AI model" boundary.

`service/src/api/v1/dbms.rs`'s `sessions` handler — the one perfect
semantic fit among all three FR-DBSEC-001 slices shipped so far, since
it already surfaces `client_addr` directly — gains a best-effort step,
same resilience posture as U55/U56.

**A real, accepted tradeoff, named explicitly**: on a fresh deployment,
every already-connected client address is "new" to this service's own
tracking on its first-ever poll, so an initial burst of detections is
expected, not a bug — the same characteristic `detect_untrusted_device`
has always had for already-attached devices, never previously written
down. Worth naming here since this is the first time this detector
*shape* gets wired into a real running path at all (`detect_untrusted_
device` itself, per ADR 0060's own finding, has still never been wired
into `service`).

## A real discovery while writing the service-level test

The first version asserted `client_addr == "127.0.0.1"`. It failed for
real: `list_sessions`'s own pre-existing `client_addr::text` cast
(unmodified by this unit) reports a host address as `"127.0.0.1/32"`,
not the bare IP string — casting PostgreSQL's `inet` type to text
preserves its netmask. Pre-existing wire behavior, not something this
unit changes; fixed the test's own assertion to match reality rather
than changing production code to match an assumption.

## Verification (real, not simulated)

- `core/src/security/detector.rs`'s own unit tests (6 cases total, 1
  new).
- `core/tests/security_detector_unusual_client_address_test.rs` (3
  tests, mirrors `security_detector_untrusted_device_test.rs`'s exact
  shape): repository durability across a fresh connection; the pure
  detector's fire/no-fire rules; one real end-to-end proof producing a
  real `security_events`/`security_incidents` row.
- `service/tests/dbms_endpoints.rs`'s new
  `sessions_polling_a_real_new_tcp_client_produces_a_real_security_
  incident`: real TCP listening enabled on a real `TestPostgres`
  instance, a real independent TCP connection (the "unusual client"
  itself) kept alive through a real `/sessions` poll, producing a real,
  queryable `MEDIUM`-severity incident over `GET /api/v1/security/
  incidents`. Passed on the first corrected attempt and 3/3 reruns.
- Full workspace: `cargo build/test/clippy -D warnings/fmt --check`
  clean (29/29 in `dbms_endpoints.rs`). `repository_migrations_up_
  down.rs`'s `EXPECTED_TABLES` updated. Both grep gates clean (new SQL
  lives in already-allowed `core::repository`). Schema-drift clean (no
  `contracts` change). `cargo deny check`/`cargo audit` clean. No
  leaked PostgreSQL processes.

## Consequences

- SRS FR-DBSEC-001(d) is now real and live: a genuinely new client
  address is detected and correlated into a real security incident the
  next time `/api/v1/dbms/sessions` is polled.
- Three of five FR-DBSEC-001 sub-items are now closed (U55's
  configuration changes, U56's superuser grants, this unit's unusual
  client addresses). **Only two remain, both explicitly deferred and
  meaningfully larger than any slice shipped so far**: permission/ACL-
  change detection (needs a real design decision on grant granularity)
  and repeated-authentication-failure detection (needs server-log
  ingestion, a fundamentally different I/O class this codebase's
  SQL-view-only `core::dbms` design doesn't support today).
