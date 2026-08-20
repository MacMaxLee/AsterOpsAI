import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/security_incidents_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

SecurityIncidentSummary fakeIncident({int id = 1}) => SecurityIncidentSummary(
  closedAt: null,
  eventCount: 3,
  id: id,
  openedAt: DateTime.utc(2026, 1, 1, 9),
  severity: 'HIGH',
  status: 'OPEN',
  summary: 'previously-unseen removable device attached',
);

Future<void> pumpScreen(WidgetTester tester, dynamic transport) => pumpApp(
  tester,
  const Scaffold(body: SecurityIncidentsScreen()),
  transport: transport,
);

void main() {
  testWidgets('a scripted open incident renders severity/status/count', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/security/incidents',
      ApiOk(jsonEncode(okEnvelopeJson([fakeIncident().toJson()]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(
      find.text('previously-unseen removable device attached'),
      findsOneWidget,
    );
    expect(find.textContaining('OPEN'), findsOneWidget);
    expect(find.textContaining('3 event'), findsOneWidget);
  });

  testWidgets('an empty incident list shows the real empty state', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/security/incidents',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.text('Nothing to show'), findsOneWidget);
  });

  testWidgets('suppressing a detector with no resource sends resource: null', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/security/incidents',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    await tester.tap(find.text('Suppress a detector'));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.widgetWithText(TextFormField, 'Detector ID'),
      'host.untrusted_device_attached',
    );
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Reason'),
      'known false positive on this fleet',
    );

    transport.queuePost(
      '/api/v1/security/suppress',
      ApiOk(jsonEncode(okEnvelopeJson(null))),
    );

    await tester.tap(find.text('Suppress').last);
    await tester.pumpAndSettle();

    final posted = transport.postedRequests.single;
    expect(posted.requestedPath, '/api/v1/security/suppress');
    final body = jsonDecode(posted.body) as Map<String, dynamic>;
    expect(body['detector_id'], 'host.untrusted_device_attached');
    expect(body['resource'], isNull);
    expect(body['reason'], 'known false positive on this fleet');
    expect(body['created_by'], 'console-operator');

    expect(find.text('Detector suppressed'), findsOneWidget);
  });

  testWidgets('a half-filled resource (kind but no name) blocks submission', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/security/incidents',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    await tester.tap(find.text('Suppress a detector'));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.widgetWithText(TextFormField, 'Detector ID'),
      'dbms.superuser_override_used',
    );
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Reason'),
      'expected on this connection',
    );
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Resource kind (optional)'),
      'INFRASTRUCTURE',
    );
    // Resource name deliberately left blank.

    await tester.tap(find.text('Suppress').last);
    await tester.pumpAndSettle();

    expect(
      find.text(
        'Resource kind and name must both be given, or both left blank',
      ),
      findsOneWidget,
    );
    // Never sent to the server at all.
    expect(transport.postedRequests, isEmpty);
  });
}
