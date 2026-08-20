import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/tuning_history_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

TuningPlanSummary fakePlan({int id = 1, DateTime? completedAt}) =>
    TuningPlanSummary(
      candidatesJson: '[]',
      completedAt: completedAt,
      createdAt: DateTime.utc(2026, 1, 1, 10),
      id: id,
      mode: 'RECOMMEND_ONLY',
      profile: 'BALANCED',
      status: completedAt == null ? 'IN_FLIGHT' : 'COMPLETED',
      targetIdentityJson: '{"kind":"PROCESS","pid":1,"start_time_ticks":1}',
    );

void main() {
  testWidgets('a scripted tuning plan renders profile/mode/status', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/tuning/plans',
      ApiOk(jsonEncode(okEnvelopeJson([fakePlan().toJson()]))),
    );
    await pumpApp(
      tester,
      const Scaffold(body: TuningHistoryScreen()),
      transport: transport,
    );
    await tester.pump();

    expect(find.text('BALANCED · RECOMMEND_ONLY'), findsOneWidget);
    expect(find.textContaining('IN_FLIGHT'), findsOneWidget);
  });

  testWidgets('an empty plan list shows the real empty state', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/tuning/plans',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpApp(
      tester,
      const Scaffold(body: TuningHistoryScreen()),
      transport: transport,
    );
    await tester.pump();

    expect(find.text('Nothing to show'), findsOneWidget);
  });

  testWidgets(
    'a plan with no completedAt never shows a fabricated completion time',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/tuning/plans',
        ApiOk(jsonEncode(okEnvelopeJson([fakePlan(id: 1).toJson()]))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: TuningHistoryScreen()),
        transport: transport,
      );
      await tester.pump();

      expect(find.textContaining('completed'), findsNothing);
    },
  );

  testWidgets('a completed plan shows its real completion time', (
    tester,
  ) async {
    final transport = createFakeTransport();
    final completedAt = DateTime.utc(2026, 1, 1, 11, 30);
    transport.queue(
      '/api/v1/tuning/plans',
      ApiOk(
        jsonEncode(
          okEnvelopeJson([fakePlan(id: 2, completedAt: completedAt).toJson()]),
        ),
      ),
    );
    await pumpApp(
      tester,
      const Scaffold(body: TuningHistoryScreen()),
      transport: transport,
    );
    await tester.pump();

    expect(find.textContaining('COMPLETED'), findsOneWidget);
    expect(find.textContaining('completed'), findsOneWidget);
  });
}
