import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/policy_inbox_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

PendingActionSummary fakeAction({int id = 1}) => PendingActionSummary(
  actionType: 'host.set_process_priority',
  approvalExpiresAt: DateTime.utc(2026, 1, 1, 12),
  createdAt: DateTime.utc(2026, 1, 1, 11),
  id: id,
  requestedBy: 'tuning-engine',
  resourceKind: 'PROCESS',
  resourceName: 'sleep-test-target',
  riskClassification: 'MEDIUM',
);

void main() {
  testWidgets('a scripted pending action renders with the right fields', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/policy/pending',
      ApiOk(jsonEncode(okEnvelopeJson([fakeAction().toJson()]))),
    );
    await pumpApp(
      tester,
      const Scaffold(body: PolicyInboxScreen()),
      transport: transport,
    );
    await tester.pump();

    expect(find.text('host.set_process_priority'), findsOneWidget);
    expect(find.textContaining('sleep-test-target'), findsOneWidget);
    expect(find.textContaining('tuning-engine'), findsOneWidget);
    expect(find.textContaining('MEDIUM'), findsOneWidget);
    expect(find.text('Grant'), findsOneWidget);
    expect(find.text('Reject'), findsOneWidget);
  });

  testWidgets('an empty pending list shows the real empty state', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/policy/pending',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpApp(
      tester,
      const Scaffold(body: PolicyInboxScreen()),
      transport: transport,
    );
    await tester.pump();

    expect(find.text('Nothing to show'), findsOneWidget);
  });

  testWidgets(
    'tapping Grant sends a real POST and the row disappears once granted',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/policy/pending',
        ApiOk(jsonEncode(okEnvelopeJson([fakeAction(id: 42).toJson()]))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: PolicyInboxScreen()),
        transport: transport,
      );
      await tester.pump();
      expect(find.text('host.set_process_priority'), findsOneWidget);

      transport.queuePost(
        '/api/v1/policy/42/grant',
        ApiOk(jsonEncode(okEnvelopeJson(null))),
      );
      // The invalidated provider re-fetches immediately — script the
      // post-grant list as genuinely empty (the real server-side
      // consequence of a successful grant), not an optimistic local
      // removal.
      transport.queue(
        '/api/v1/policy/pending',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );

      await tester.tap(find.text('Grant'));
      await tester.pump();
      await tester.pump();

      final posted = transport.postedRequests.single;
      expect(posted.requestedPath, '/api/v1/policy/42/grant');
      expect(jsonDecode(posted.body), {'granted_by': 'console-operator'});

      expect(find.text('host.set_process_priority'), findsNothing);
      expect(find.text('Nothing to show'), findsOneWidget);
    },
  );

  testWidgets(
    'tapping Reject with a real 400 shows an inline error and keeps the row',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/policy/pending',
        ApiOk(jsonEncode(okEnvelopeJson([fakeAction(id: 7).toJson()]))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: PolicyInboxScreen()),
        transport: transport,
      );
      await tester.pump();

      transport.queuePost(
        '/api/v1/policy/7/reject',
        ApiOk(
          jsonEncode(
            errEnvelopeJson(
              const ApiErrorBadRequest(
                message:
                    'action 7 is not in the expected status: expected '
                    'PENDING_APPROVAL, actual DENIED',
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Reject'));
      await tester.pump();
      await tester.pump();

      expect(
        find.textContaining('action 7 is not in the expected status'),
        findsOneWidget,
      );
      // The row is never silently dropped just because the call failed.
      expect(find.text('host.set_process_priority'), findsOneWidget);
    },
  );
}
