# 0021 — Console: Policy Approval Inbox, `postRaw`, and the Mutation-Provider Pattern

## Status

Accepted (unit U16).

## Context

U13/U14/U15 each explicitly deferred console work as its own follow-up.
This unit is that follow-up for the first of the three domains: a real
console screen for U13's policy approval inbox. Console work is
genuinely testable here — `/home/parallels/flutter/bin/flutter` is a
real, working Flutter SDK in this sandbox (`flutter analyze`/`flutter
test` both confirmed clean before this unit started), correcting an
earlier research pass's mistaken "Flutter isn't installed" finding.

## Decision

**Console codegen was three schemas behind, closed by this unit.**
`pending_action_summary.schema.json`/`tuning_plan_summary.schema.json`/
`security_incident_summary.schema.json` (added in U13/U14/U15) had never
been run through `console/tool/generate_models.dart` — a real,
already-existing drift that `scripts/check-console-codegen-drift.sh`
would have caught the moment anyone ran it. This unit runs the regen and
commits the result, same as every prior schema-drift resolution in this
project (Rust's own `emit-schemas.sh`/`check-schema-drift.sh` pattern).
No generator changes were needed — the three new types are plain objects
with primitives, strings, and one nullable `DateTime`, shapes the
generator already handled.

**`LocalTransport` gained `postRaw` — its first mutating call.** Every
prior screen only ever read; grant/reject are this project's first
console-initiated mutation. Added in three places: the abstract
interface, `UnixSocketTransport` (via `HttpClient.postUrl`, the same
connection machinery `getRaw` already uses), and `FakeTransport` (a
second, independent FIFO queue via `queuePost`, kept separate from the
existing `getRaw` queues so a screen that both polls a list and posts a
mutation can script each without cross-talk).

**No new abstraction for the mutation itself — `ApiClient.grantAction`/
`rejectAction` are plain one-shot calls, and the screen invalidates
`pendingActionsProvider` directly on success.** Every existing provider
is a `StreamProvider` over a `PolledRepository` (continuous polling);
grant/reject aren't something to poll. Riverpod's own `ref.invalidate`
is the idiom for "this provider's data is now stale" — the row
disappears once the server itself confirms the new status via a real
re-fetch, not an optimistic local removal. No new repository/provider
type was invented for this one shape.

**`grantAction`/`rejectAction` needed a genuinely separate envelope-decode
path, not a forced reuse of `_decodeEnvelope<T>`.** Both return `()` on
the Rust side, which serializes as `data: null` even on *success* —
`_decodeEnvelope<T>`'s existing rule ("`success` with null `data` is
malformed") is correct for every other endpoint but wrong for these two.
`_decodeEnvelopeSuccess` checks only the envelope's own `success`/`error`
fields, never `data`. This is a real, deliberate divergence, documented
here rather than silently special-cased inline.

**`pubspec.yaml`'s description was corrected, not left stale.** It said
"a read-only thin client over the core API" — accurate only because no
mutation endpoint existed yet. SRS §2's own product vision explicitly
includes a policy-gated action executor in scope for v1 ("where safe and
approved, it can also change a small number of tunable settings"), and
FR-CONSOLE-001/002 constrain what the console *computes* (no business
logic, no client-side reclassification) — not whether it can call a
mutating endpoint. The description now reads "a thin client over the
core API."

**No auth/session system exists anywhere in this project yet.** The
screen names its own operator (`'console-operator'`) the same plain way
`core/tests`' own `approval::grant` calls already do — a known,
unaddressed gap, not a design decision this unit makes.

**Widget tests pump `Scaffold(body: PolicyInboxScreen())` directly,
not the full `RootGate` → `AppShell` flow every prior test used.** A
real, first-time discovery: `ListTile` (used by `_PendingActionRow`,
the same widget `DevicesScreen`/`ProcessesScreen` already use with no
widget test of their own) requires a `Material` ancestor, which
`AppShell`'s own `Scaffold` normally provides. Testing the screen in
isolation needed the same real ancestor a production render would have —
not a workaround, a faithful minimal reproduction of the real render
context.

## Consequences

- Tuning-history and security-incidents screens are real, separate
  follow-up units, reusing this exact pattern (a `StreamProvider` for
  the list; for security's `suppress`, the same `postRaw`/
  `_decodeEnvelopeSuccess`/`ref.invalidate` shape this unit already
  proved).
- `postRaw`/`_decodeEnvelopeSuccess` are now real, reusable pieces of
  infrastructure — a future mutation with a *meaningful* response body
  would still go through the ordinary `_decodeEnvelope<T>` path, not
  this one; the two decode paths solve genuinely different response
  shapes, not a preference between them.
- `DevicesScreen`/`ProcessesScreen` still have no widget test of their
  own (this unit didn't add one) — the `Material`-ancestor requirement
  this unit discovered would apply to them too, whenever that gap is
  closed.
