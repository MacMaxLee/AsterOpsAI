import 'dart:convert';

import 'package:console/api/api_failure.dart';
import 'package:console/api/api_result.dart';
import 'package:console/screens/root_gate.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

void main() {
  testWidgets(
    'shows a connecting/failure state before any health reply is scripted',
    (tester) async {
      final transport = createFakeTransport();
      await pumpApp(tester, const RootGate(), transport: transport);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('renders the app shell once health reports a matching version', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/health',
      ApiOk(jsonEncode(okEnvelopeJson(fakeHealth().toJson()))),
    );
    await pumpApp(tester, const RootGate(), transport: transport);
    await tester.pump();

    expect(find.byIcon(Icons.dashboard_outlined), findsOneWidget);
  });

  testWidgets('an incompatible api_version blocks the whole console', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/health',
      ApiOk(jsonEncode(okEnvelopeJson(fakeHealth(apiVersion: 'v99').toJson()))),
    );
    await pumpApp(tester, const RootGate(), transport: transport);
    await tester.pump();

    expect(find.byIcon(Icons.system_update), findsOneWidget);
    // The nav shell — and therefore every data screen behind it — must not
    // be reachable at all, not merely hidden.
    expect(find.byIcon(Icons.dashboard_outlined), findsNothing);
  });

  testWidgets('a timeout on the very first connection attempt is distinct', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue('/api/v1/health', const ApiErr(ApiFailureTimeout()));
    await pumpApp(tester, const RootGate(), transport: transport);
    await tester.pump();

    expect(find.textContaining('too long'), findsOneWidget);
  });

  testWidgets('core-unavailable on first connect shows a retry affordance', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/health',
      const ApiErr(ApiFailureUnavailable('connection refused')),
    );
    await pumpApp(tester, const RootGate(), transport: transport);
    await tester.pump();

    expect(find.text('Retry now'), findsOneWidget);
  });

  testWidgets('a malformed payload is decoded (not crashed on) and shown', (
    tester,
  ) async {
    final transport = createFakeTransport();
    // Realistic: this is real invalid JSON reaching the real ApiClient
    // decode path, not a failure injected above that layer.
    transport.queue('/api/v1/health', const ApiOk('not valid json {'));
    await pumpApp(tester, const RootGate(), transport: transport);
    await tester.pump();

    expect(tester.takeException(), isNull);
    expect(find.textContaining('unexpected response'), findsOneWidget);
  });

  testWidgets(
    'losing a connection that was previously up reads as reconnecting, not a fresh error',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/health',
        ApiOk(jsonEncode(okEnvelopeJson(fakeHealth().toJson()))),
      );
      await pumpApp(tester, const RootGate(), transport: transport);
      await tester.pump();
      expect(find.byIcon(Icons.dashboard_outlined), findsOneWidget);

      transport.queue(
        '/api/v1/health',
        const ApiErr(ApiFailureUnavailable('connection reset')),
      );
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();

      expect(find.textContaining('Reconnecting'), findsOneWidget);
      // Reconnecting is reassuring, not the raw first-connect diagnostic.
      expect(find.text('Retry now'), findsNothing);
    },
  );
}
