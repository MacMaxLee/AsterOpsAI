import 'dart:convert';

import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/correlation_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

CorrelationResult fakeCorrelationResult({
  List<Hypothesis> ranked = const [],
  List<RuledOut> ruledOut = const [],
}) => CorrelationResult(
  ranked: ranked,
  ruledOut: ruledOut,
  windowEnd: DateTime.utc(2026, 1, 1, 10),
  windowStart: DateTime.utc(2026, 1, 1, 9, 55),
);

void main() {
  testWidgets(
    'a ranked DB_LOCKS hypothesis renders its cause/confidence/evidence',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/correlation',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              fakeCorrelationResult(
                ranked: [
                  Hypothesis(
                    cause: RootCause.dbLocks,
                    confidence: 0.75,
                    evidence: [
                      Evidence(
                        metric: 'blocking_lock_edges',
                        observed: 6,
                        threshold: 5,
                        windowStart: DateTime.utc(2026, 1, 1, 9, 55),
                        windowEnd: DateTime.utc(2026, 1, 1, 10),
                      ),
                    ],
                  ),
                ],
                ruledOut: const [
                  RuledOut(
                    cause: RootCause.hostCpu,
                    reason: 'no host evidence available',
                  ),
                ],
              ).toJson(),
            ),
          ),
        ),
      );
      await pumpApp(
        tester,
        const Scaffold(body: CorrelationScreen()),
        transport: transport,
      );
      await tester.pump();

      expect(find.text('Database locks'), findsOneWidget);
      expect(find.textContaining('Confidence 75%'), findsOneWidget);
      expect(
        find.textContaining('blocking_lock_edges: 6.00 (threshold 5.00)'),
        findsOneWidget,
      );
      expect(find.text('Host CPU'), findsOneWidget);
      expect(find.text('no host evidence available'), findsOneWidget);
    },
  );

  testWidgets(
    'an all-ruled-out verdict renders honestly with no fabricated ranked cause',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/correlation',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              fakeCorrelationResult(
                ruledOut: const [
                  RuledOut(
                    cause: RootCause.dbLocks,
                    reason: 'no DB evidence available',
                  ),
                ],
              ).toJson(),
            ),
          ),
        ),
      );
      await pumpApp(
        tester,
        const Scaffold(body: CorrelationScreen()),
        transport: transport,
      );
      await tester.pump();

      expect(find.text('Ranked hypotheses'), findsOneWidget);
      expect(find.text('Nothing to show'), findsOneWidget);
      expect(find.text('Database locks'), findsOneWidget);
      expect(find.text('no DB evidence available'), findsOneWidget);
    },
  );
}
