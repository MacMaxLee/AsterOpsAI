import 'dart:convert';

import 'package:console/api/api_failure.dart';
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

AiExplanation fakeAiExplanation() => const AiExplanation(
  summary: 'No sustained root cause identified.',
  observations: [
    Observation(text: 'Every domain is within its normal range.', metrics: []),
  ],
  recommendations: [
    Recommendation(text: 'No action needed.', metrics: [], candidateRef: null),
  ],
  risk: RiskLevel.low,
  confidence: 0.8,
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

  testWidgets(
    'a realistic multi-cause verdict fits a laptop viewport with no overflow '
    '(REQUIREMENTS #5: no scrolling, no toggle)',
    (tester) async {
      final originalSize = tester.view.physicalSize;
      final originalRatio = tester.view.devicePixelRatio;
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(() {
        tester.view.physicalSize = originalSize;
        tester.view.devicePixelRatio = originalRatio;
      });

      Evidence evidenceAt(String metric, double observed, double threshold) =>
          Evidence(
            metric: metric,
            observed: observed,
            threshold: threshold,
            windowStart: DateTime.utc(2026, 1, 1, 9, 55),
            windowEnd: DateTime.utc(2026, 1, 1, 10),
          );

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
                    confidence: 0.35,
                    evidence: [
                      evidenceAt('blocking_lock_edges', 6, 5),
                      evidenceAt('deadlocks_since_reset', 0, 1),
                      evidenceAt('long_transaction_count', 0, 1),
                    ],
                  ),
                  Hypothesis(
                    cause: RootCause.clientSideApplication,
                    confidence: 0.23,
                    evidence: const [],
                  ),
                ],
                ruledOut: const [
                  RuledOut(
                    cause: RootCause.dbConfiguration,
                    reason: 'DB checks show no sustained problem',
                  ),
                  RuledOut(
                    cause: RootCause.connectionExhaustion,
                    reason: 'DB checks show no sustained problem',
                  ),
                  RuledOut(
                    cause: RootCause.slowSql,
                    reason: 'DB checks show no sustained problem',
                  ),
                  RuledOut(
                    cause: RootCause.hostCpu,
                    reason: 'no sustained signal',
                  ),
                  RuledOut(
                    cause: RootCause.hostMemory,
                    reason: 'no sustained signal',
                  ),
                  RuledOut(
                    cause: RootCause.storageLatency,
                    reason: 'no sustained signal',
                  ),
                  RuledOut(
                    cause: RootCause.network,
                    reason: 'no sustained signal',
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

      expect(tester.takeException(), isNull);
      expect(find.byType(SingleChildScrollView), findsNothing);
      expect(find.byType(ListView), findsNothing);
      expect(find.text('Database locks'), findsOneWidget);
      expect(find.text('Client-side application'), findsOneWidget);
      expect(find.text('Network'), findsOneWidget);
    },
  );

  testWidgets(
    'tapping Explain with AI opens a dialog and renders a real Supported '
    'explanation',
    (tester) async {
      final transport = createFakeTransport();
      // `/explain` is queued first: `FakeTransport` matches by path
      // *prefix*, first-inserted-key-wins, and `/analysis/correlation/
      // explain` also starts with `/analysis/correlation` — queuing the
      // more specific path first is this codebase's own established
      // convention for exactly this collision (see
      // `host_analysis_screen_test.dart`'s own explain tests).
      transport.queue(
        '/api/v1/analysis/correlation/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForAiExplanationSupported(value: fakeAiExplanation())
                  .toJson(),
            ),
          ),
        ),
      );
      transport.queue(
        '/api/v1/analysis/correlation',
        ApiOk(jsonEncode(okEnvelopeJson(fakeCorrelationResult().toJson()))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: CorrelationScreen()),
        transport: transport,
      );
      await tester.pump();

      expect(find.text('Explain with AI'), findsOneWidget);
      await tester.tap(find.byKey(const Key('correlationExplainTrigger')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(find.text('No sustained root cause identified.'), findsOneWidget);
      expect(
        find.text('•  Every domain is within its normal range.'),
        findsOneWidget,
      );
      expect(find.text('→  No action needed.'), findsOneWidget);
      expect(find.textContaining('LOW'), findsOneWidget);
    },
  );

  testWidgets(
    'a real Unavailable gated correlation explanation shows the real reason '
    'via the shared vocabulary',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/correlation/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              const GatedValueForAiExplanationUnavailable(
                reason: 'AI explanation unavailable',
              ).toJson(),
            ),
          ),
        ),
      );
      transport.queue(
        '/api/v1/analysis/correlation',
        ApiOk(jsonEncode(okEnvelopeJson(fakeCorrelationResult().toJson()))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: CorrelationScreen()),
        transport: transport,
      );
      await tester.pump();

      await tester.tap(find.byKey(const Key('correlationExplainTrigger')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(find.text('Not available'), findsOneWidget);
      expect(find.text('AI explanation unavailable'), findsOneWidget);
    },
  );

  testWidgets(
    'a real transport-level error on correlation explain shows the real '
    'failure message with a working retry',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/correlation/explain',
        const ApiErr(ApiFailureUnavailable('connection refused')),
      );
      transport.queue(
        '/api/v1/analysis/correlation',
        ApiOk(jsonEncode(okEnvelopeJson(fakeCorrelationResult().toJson()))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: CorrelationScreen()),
        transport: transport,
      );
      await tester.pump();

      await tester.tap(find.byKey(const Key('correlationExplainTrigger')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(
        find.text(
          "The AsterOpsAI core service isn't reachable. It may have "
          'stopped, or the socket may be missing. The console will keep '
          'retrying automatically.',
        ),
        findsOneWidget,
      );

      transport.queue(
        '/api/v1/analysis/correlation/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForAiExplanationSupported(value: fakeAiExplanation())
                  .toJson(),
            ),
          ),
        ),
      );
      // Two matches now: the outer trigger (behind the still-open
      // dialog) and the dialog's own retry button — scoped to the
      // dialog specifically, since that's the one under test here.
      await tester.tap(
        find.descendant(
          of: find.byType(Dialog),
          matching: find.text('Explain with AI'),
        ),
      );
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(find.text('No sustained root cause identified.'), findsOneWidget);
    },
  );
}
