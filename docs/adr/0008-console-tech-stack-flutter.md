# 0008 — Console Tech Stack: Flutter, Not a Web/Electron/Tauri Client

## Status

Accepted (unit U3).

## Context

U0 explicitly deferred the console's tech stack — `console/README.md` said
only "not yet decided... owned by unit U3." TRS §15 already commits to the
console's *shape* without naming a framework: "Layered: screens → providers
→ repositories → API client." U3's own SCOPE line names `/flutter_app/**` as
the alternate path "if the Flutter path was chosen," signalling this was
already the anticipated default, not a fully open choice.

The decision is also constrained by ADR 0001: the core's local API is served
over a Unix domain socket (Linux/macOS) or named pipe (Windows), with no TCP
fallback, ever. A browser cannot open a raw Unix domain socket or named pipe
— there is no Web API surface for it — so a plain web app (and, by
extension, most of Electron/Tauri's usual "web frontend" model) cannot speak
to the core's real transport without an intermediary bridge process, which
would reopen exactly the "reachable by other local processes/users" problem
ADR 0001 was written to close. This rules out a browser-hosted UI as a
serious option regardless of framework preference.

## Decision

**Flutter**, at `console/` (not `console/flutter_app/`, matching the
existing placeholder path). Concretely:

- **State management**: `flutter_riverpod` — TRS §15's "providers" layer is
  Riverpod's own vocabulary, and it's fully testable without a live widget
  tree (used throughout `test/` and the live E2E script alike).
- **Transport**: `dart:io`'s `HttpClient(connectionFactory: ...)` hook,
  pointed at `Socket.startConnect` against a Unix-domain `InternetAddress`
  (`lib/api/unix_socket_transport.dart`). This is a documented, real Dart
  SDK mechanism, not a workaround — the rest of `HttpClient`'s request/
  response handling is reused unmodified. Verified for real against the
  live `ai-ops-core` binary, including killing and restarting it mid-session
  (`integration/reconnect_live_test.dart`), not just asserted to compile.
  Windows named pipes have no `dart:io` equivalent; that implementation is
  explicitly left to U12, behind the same `LocalTransport` interface this
  unit defines.
- **Model codegen**: a small custom generator
  (`console/tool/generate_models.dart`), not a general tool like
  `quicktype` — none was available in this environment (no Node/npm), and a
  purpose-built generator can rely on the exact, small set of draft-07
  shapes `schemars` actually emits (confirmed by reading every committed
  schema file before writing it — see the generator's own header comment)
  rather than handling arbitrary JSON Schema. `envelope_*.schema.json` is
  deliberately *not* generated from — every envelope is the same
  `{success, timestamp, request_id, data, error}` shape monomorphized per
  data type, so one hand-written generic `Envelope<T>`
  (`lib/api/envelope.dart`) covers all of them, mirroring how
  `contracts::Envelope<T>` is itself hand-written Rust on the server side.
- **i18n**: Flutter's built-in `flutter_localizations` + `intl`/ARB tooling
  (`lib/l10n/app_{en,zh,zh_Hant}.arb`) — not committed to git (regenerated
  automatically by `flutter pub get`/`flutter test`/`flutter build` via
  `flutter: generate: true`, unlike the schema-derived models, which need an
  explicit run and are therefore committed with a drift gate).

## Consequences

- This dev environment has no Flutter SDK, and no `clang`/`cmake`/
  `ninja-build`/`pkg-config`/`libgtk-3-dev` (needed only for
  `flutter build/run -d linux`, the real windowed GTK embedder) — `sudo`
  needs an interactive terminal here, the same blocker U0 hit installing
  `git`. This did **not** block U3: `flutter test`/`flutter analyze`/
  `dart format`/the model codegen all run on a bundled headless Dart-VM
  harness with no native windowing dependency at all, and the live
  reconnect check runs as a plain `dart run` script hitting the real
  transport/API-client layer directly, no widget tree involved. A full
  visual smoke test of the compiled windowed app needs that toolchain
  installed by the user on their own machine — a real, acknowledged gap,
  not a silently skipped one.
- `.github/workflows/ci.yml`'s new `console-*` jobs follow the same
  reasoning as `check-windows`/`check-macos` (ADR 0007's CI note): nothing
  they run needs a native desktop toolchain, so plain `ubuntu-latest` with
  `subosito/flutter-action` is sufficient — no native-runner cost the way
  U2's SQLite C build required.
- U12 ("Windows support") inherits a `LocalTransport` interface already
  shaped for a second implementation, but still has real work to do:
  `dart:io` has no named-pipe primitive, so `WindowsNamedPipeTransport` will
  need either Dart FFI against the Win32 API or a small native plugin —
  this ADR intentionally doesn't pre-design that, per the project's
  "don't build ahead of the unit that needs it" convention.
- A future unit that wants a different console codegen strategy (e.g. if
  `quicktype` or an equivalent becomes available and the hand-rolled
  generator becomes a maintenance burden) writes a new ADR rather than
  quietly replacing `tool/generate_models.dart`.
