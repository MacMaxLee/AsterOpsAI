import 'package:console/l10n/app_localizations.dart';
import 'package:console/providers/settings_provider.dart';
import 'package:console/providers/transport_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'fake_transport.dart';

/// The very first frame already triggers every provider that's watched on
/// it (health, and whatever the default screen reads) — their fetches read
/// `FakeTransport`'s queues synchronously as part of that build. So a
/// response has to be queued on a transport that exists *before*
/// `pumpWidget` runs, not after — this only creates the transport; call
/// `pumpApp` to actually build the tree once it's scripted.
FakeTransport createFakeTransport() => FakeTransport();

Future<void> pumpApp(
  WidgetTester tester,
  Widget child, {
  required FakeTransport transport,
  List<Override> extraOverrides = const [],
}) async {
  SharedPreferences.setMockInitialValues({});
  final prefs = await SharedPreferences.getInstance();

  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        transportProvider.overrideWithValue(transport),
        sharedPreferencesProvider.overrideWithValue(prefs),
        ...extraOverrides,
      ],
      child: MaterialApp(
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        home: child,
      ),
    ),
  );
}
