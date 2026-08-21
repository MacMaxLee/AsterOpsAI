import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/database_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

SessionInfo fakeSession() => SessionInfo(
  clientAddr: '10.0.0.5',
  database: 'appdb',
  pid: 4242,
  query: 'SELECT 1',
  queryStart: DateTime.utc(2026, 1, 1, 9),
  state: SessionState.active,
  username: 'app',
  xactStart: null,
);

LockEdge fakeLock() => const LockEdge(
  blockedPid: 501,
  blockedQuery: 'UPDATE t SET x = 1 WHERE id = 1',
  blockingPid: 500,
  blockingQuery: 'SELECT id FROM t WHERE id = 1 FOR UPDATE',
  lockType: 'transactionid',
);

Future<void> pumpScreen(WidgetTester tester, dynamic transport) => pumpApp(
  tester,
  const Scaffold(body: DatabaseScreen()),
  transport: transport,
);

void main() {
  testWidgets(
    'a scripted session renders its real pid/username/database/state',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson([fakeSession().toJson()]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('4242'), findsOneWidget);
      expect(find.textContaining('app @ appdb'), findsOneWidget);
      expect(find.textContaining('ACTIVE'), findsOneWidget);
      expect(find.textContaining('SELECT 1'), findsOneWidget);
    },
  );

  testWidgets(
    'a scripted lock renders its real blocked/blocking pids and lock type',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson([fakeLock().toJson()]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('PID 501 blocked by 500'), findsOneWidget);
      expect(find.textContaining('transactionid'), findsOneWidget);
    },
  );

  testWidgets('an empty response for both sections shows the real empty state '
      'independently', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/dbms/sessions',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    transport.queue(
      '/api/v1/dbms/locks',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.text('Nothing to show'), findsNWidgets(2));
  });
}
