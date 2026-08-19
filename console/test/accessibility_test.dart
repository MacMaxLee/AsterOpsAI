import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/screens/root_gate.dart';
import 'package:console/screens/settings_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

void main() {
  testWidgets('Settings screen meets contrast and tap-target guidelines', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    final transport = createFakeTransport();
    await pumpApp(
      tester,
      const Scaffold(body: SettingsScreen()),
      transport: transport,
    );
    await tester.pump();

    await expectLater(tester, meetsGuideline(textContrastGuideline));
    await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
    await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));

    handle.dispose();
  });

  testWidgets(
    'Dashboard, once connected, meets contrast and tap-target guidelines',
    (tester) async {
      final handle = tester.ensureSemantics();
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/health',
        ApiOk(jsonEncode(okEnvelopeJson(fakeHealth().toJson()))),
      );
      transport.queue(
        '/api/v1/system/status',
        ApiOk(jsonEncode(okEnvelopeJson(fakeSystemStatusResponse().toJson()))),
      );
      transport.queue(
        '/api/v1/cpu',
        ApiOk(jsonEncode(okEnvelopeJson(fakeCpuSnapshot().toJson()))),
      );
      transport.queue(
        '/api/v1/memory',
        ApiOk(jsonEncode(okEnvelopeJson(fakeMemorySnapshot().toJson()))),
      );
      await pumpApp(tester, const RootGate(), transport: transport);
      await tester.pump();

      await expectLater(tester, meetsGuideline(textContrastGuideline));
      await expectLater(tester, meetsGuideline(androidTapTargetGuideline));

      handle.dispose();
    },
  );
}
