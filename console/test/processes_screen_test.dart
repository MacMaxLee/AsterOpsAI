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

ActionProposalOutcome fakeProposalOutcome() =>
    const ActionProposalOutcome(rowId: 9, status: 'PENDING_APPROVAL');

ResumableActionSummary fakeResumable() => ResumableActionSummary(
  actionType: 'security.suspend_process',
  executedAt: DateTime.utc(2026, 1, 1, 9),
  rowId: 11,
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

  testWidgets(
    'the suspend confirmation dialog shows the row\'s real comm and pid',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.pause_circle_outlined));
      await tester.pumpAndSettle();

      expect(
        find.text('example (PID 4242) will be frozen once this is approved.'),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'confirming suspend posts the real security.suspend_process proposal '
    'and renders the real status/row_id verbatim',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.pause_circle_outlined));
      await tester.pumpAndSettle();

      transport.queuePost(
        '/api/v1/actions/propose',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProposalOutcome().toJson()))),
      );

      await tester.tap(find.text('Suspend'));
      await tester.pumpAndSettle();

      final posted = transport.postedRequests.single;
      expect(posted.requestedPath, '/api/v1/actions/propose');
      final body = jsonDecode(posted.body) as Map<String, dynamic>;
      expect(body['action_type'], 'security.suspend_process');
      expect(body['pid'], 4242);
      expect(body['start_time_ticks'], 12345);
      expect(body['resource_name'], 'example');
      expect(body['requested_by'], 'console-operator');
      expect(body.containsKey('parameters'), isFalse);

      expect(find.text('Action proposed'), findsOneWidget);
      expect(find.text('PENDING_APPROVAL (row 9)'), findsOneWidget);
    },
  );

  testWidgets(
    'a real error suspending shows an inline message and keeps the dialog '
    'open, not a crash or a silent dismiss',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.pause_circle_outlined));
      await tester.pumpAndSettle();

      transport.queuePost(
        '/api/v1/actions/propose',
        ApiOk(
          jsonEncode(
            errEnvelopeJson(
              const ApiErrorBadRequest(
                message:
                    'invalid parameters for action type '
                    'security.suspend_process: unexpected extra fields',
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Suspend'));
      await tester.pumpAndSettle();

      expect(
        find.text(
          'invalid parameters for action type security.suspend_process: '
          'unexpected extra fields',
        ),
        findsOneWidget,
      );
      expect(find.text('Suspend process?'), findsOneWidget);
    },
  );

  testWidgets(
    'the resume dialog fires the lookup on open, using the row\'s real '
    'pid/start_time_ticks, and shows the real not-found message when '
    'nothing is resumable',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      transport.queue(
        '/api/v1/actions/resumable',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.play_circle_outlined));
      await tester.pumpAndSettle();

      expect(
        transport.requestedPaths.last,
        '/api/v1/actions/resumable?pid=4242&start_time_ticks=12345',
      );
      expect(
        find.text('No resumable action found for this process.'),
        findsOneWidget,
      );
      // No Resume button when there's nothing to resume.
      expect(find.text('Resume'), findsNothing);
    },
  );

  testWidgets('a found resumable action shows its real action_type and, on '
      'confirm, posts the real rollback and shows a success snackbar', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/processes',
      ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
    );
    transport.queue(
      '/api/v1/actions/resumable',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[fakeResumable().toJson()]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    await tester.tap(find.byIcon(Icons.play_circle_outlined));
    await tester.pumpAndSettle();

    expect(find.textContaining('security.suspend_process'), findsOneWidget);

    transport.queuePost(
      '/api/v1/policy/11/rollback',
      ApiOk(jsonEncode(okEnvelopeJson(null))),
    );

    await tester.tap(find.text('Resume'));
    await tester.pumpAndSettle();

    final posted = transport.postedRequests.single;
    expect(posted.requestedPath, '/api/v1/policy/11/rollback');
    final body = jsonDecode(posted.body) as Map<String, dynamic>;
    expect(body['rolled_back_by'], 'console-operator');

    expect(
      find.text('Resumed security.suspend_process (row 11)'),
      findsOneWidget,
    );
  });

  testWidgets(
    'a real error resuming shows an inline message and keeps the dialog '
    'open, not a crash or a silent dismiss',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/processes',
        ApiOk(jsonEncode(okEnvelopeJson(fakeProcessSnapshot().toJson()))),
      );
      transport.queue(
        '/api/v1/actions/resumable',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[fakeResumable().toJson()]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      await tester.tap(find.byIcon(Icons.play_circle_outlined));
      await tester.pumpAndSettle();

      transport.queuePost(
        '/api/v1/policy/11/rollback',
        ApiOk(
          jsonEncode(
            errEnvelopeJson(
              const ApiErrorBadRequest(
                message:
                    'action 11 is not in the expected status: '
                    'expected EXECUTED, actual ROLLED_BACK',
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Resume'));
      await tester.pumpAndSettle();

      expect(
        find.text(
          'action 11 is not in the expected status: '
          'expected EXECUTED, actual ROLLED_BACK',
        ),
        findsOneWidget,
      );
      expect(find.text('Resume process'), findsOneWidget);
    },
  );
}
