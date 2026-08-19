# console

The read-only operator console (unit U3) — a Flutter desktop app, thin
client only: no thresholds, no scoring, no classification of its own (SRS
FR-CONSOLE-001). See `docs/adr/0008-console-tech-stack-flutter.md` for why
Flutter, and `docs/SRS.md` §15 / `docs/TRS.md` §15–16 for the requirements
this satisfies.

## Layout

- `lib/generated/models/` — **generated, committed, do not hand-edit.**
  Regenerate with `dart run tool/generate_models.dart` after any change to
  `/schemas/*.schema.json`; CI's `console-codegen-drift` job fails if the
  committed output doesn't match a fresh run.
- `lib/api/` — the transport (Unix domain socket today; Windows named pipes
  are U12) and the typed API client. The only layer allowed to do HTTP.
- `lib/repositories/` — polling loops per data family, exposed as streams.
- `lib/providers/` — Riverpod providers wrapping the repositories, plus
  connection-status and settings state.
- `lib/screens/`, `lib/widgets/` — UI. Widgets never call the API client or
  repositories directly (TRS §15) — only through a provider.
- `lib/l10n/*.arb` — translation source (en, zh-Hant); the generated
  `AppLocalizations` Dart files are *not* committed (see `.gitignore`'s
  comment) — `flutter pub get`/`flutter test`/`flutter build` regenerate
  them automatically.
- `test/` — widget tests, including one per connection/metric error state
  and two `meetsGuideline` accessibility checks.
- `integration/reconnect_live_test.dart` — a live check against the real
  `ai-ops-core` binary (kills and restarts it, confirms the client notices
  and recovers on its own). Run with `dart run
  integration/reconnect_live_test.dart` after `cargo build --workspace`.

## Running

```
flutter pub get
flutter test
dart run integration/reconnect_live_test.dart   # after cargo build --workspace
```

`flutter run -d linux` needs a real GTK/desktop-embedder toolchain
(`clang`, `cmake`, `ninja-build`, `pkg-config`, `libgtk-3-dev`) that isn't
required for anything above.
