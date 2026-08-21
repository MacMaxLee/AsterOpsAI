import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/processes_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

Future<void> pumpScreen(WidgetTester tester, dynamic transport) => pumpApp(
  tester,
  const Scaffold(body: ProcessesScreen()),
  transport: transport,
);

TuningPlanOutcome fakeOutcome() => const TuningPlanOutcome(
  candidates: [
    TuningCandidateOutcome(
      actionType: 'host.set_process_cpu_affinity',
      outcome: 'AUTO_ALLOWED_PENDING',
      rowId: 7,
    ),
  ],
  planId: 42,
  status: 'COMPLETED',
);

void main() {
  testWidgets('a scripted process renders comm/pid/category', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/processes',
      ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.text('example'), findsOneWidget);
    expect(find.textContaining('4242'), findsOneWidget);
  });

  testWidgets('an empty process list shows the real empty state', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/processes',
      ApiOk(
        jsonEncode(
          okEnvelopeJson(
            ProcessSnapshot(
              processes: const [],
              timestamp: DateTime.now().toUtc(),
              totalCount: 0,
            ).toJson(),
          ),
        ),
      ),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.text('Nothing to show'), findsOneWidget);
  });

  testWidgets('the start-tuning-plan dialog pre-fills the resource name '
      'with the row\'s real comm', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/processes',
      ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();

    expect(find.widgetWithText(TextFormField, 'example'), findsOneWidget);
  });

  testWidgets(
    'submitting sends the row\'s real pid/start_time_ticks and the chosen '
    'profile/mode, and renders each real candidate outcome verbatim',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.tune_outlined));
      await tester.pumpAndSettle();

      transport.queuePost(
        '/api/v1/tuning/start',
        ApiOk(jsonEncode(okEnvelopeJson(fakeOutcome().toJson()))),
      );

      await tester.tap(find.text('Start plan'));
      await tester.pumpAndSettle();

      final posted = transport.postedRequests.single;
      expect(posted.requestedPath, '/api/v1/tuning/start');
      final body = jsonDecode(posted.body) as Map<String, dynamic>;
      expect(body['pid'], 4242);
      expect(body['start_time_ticks'], 12345);
      expect(body['resource_name'], 'example');
      expect(body['profile'], 'BALANCED');
      expect(body['mode'], 'RECOMMEND_ONLY');
      expect(body['requested_by'], 'console-operator');

      expect(find.text('Plan started'), findsOneWidget);
      expect(
        find.text('host.set_process_cpu_affinity — AUTO_ALLOWED_PENDING'),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'a real 400 shows an inline error and keeps the dialog open, not a '
    'crash or a silent dismiss',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.tune_outlined));
      await tester.pumpAndSettle();

      transport.queuePost(
        '/api/v1/tuning/start',
        ApiOk(
          jsonEncode(
            errEnvelopeJson(
              const ApiErrorBadRequest(
                message: 'a tuning plan is already in flight for this target',
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Start plan'));
      await tester.pumpAndSettle();

      expect(
        find.text('a tuning plan is already in flight for this target'),
        findsOneWidget,
      );
      expect(find.text('Start tuning plan'), findsOneWidget);
    },
  );
}
