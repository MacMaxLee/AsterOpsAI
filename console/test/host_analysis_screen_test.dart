import 'dart:convert';

import 'package:console/api/api_failure.dart';
import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/host_analysis_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

HostVerdict fakeVerdict({
  HostBottleneck bottleneck = HostBottleneck.cpu,
  List<DomainSignal> domainSignals = const [],
}) => HostVerdict(
  bottleneck: bottleneck,
  domainSignals: domainSignals,
  evidence: const [],
  score: 72,
  scoreVersion: 'host-v1',
);

AiExplanation fakeExplanation() => const AiExplanation(
  summary: 'The host looks CPU-bound.',
  observations: [
    Observation(text: 'CPU pressure has been sustained.', metrics: []),
  ],
  recommendations: [
    Recommendation(
      text: 'Consider closing background processes.',
      metrics: [],
      candidateRef: null,
    ),
  ],
  risk: RiskLevel.medium,
  confidence: 0.82,
);

void main() {
  testWidgets('a scripted CPU-bottleneck verdict renders bottleneck/score/'
      'domain rows', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/analysis/host',
      ApiOk(
        jsonEncode(
          okEnvelopeJson(
            fakeVerdict(
              domainSignals: const [
                DomainSignal(
                  crossedCount: 4,
                  domain: HostDomain.cpu,
                  sampleCount: 5,
                  tier: Tier.critical,
                ),
              ],
            ).toJson(),
          ),
        ),
      ),
    );
    await pumpApp(
      tester,
      const Scaffold(body: HostAnalysisScreen()),
      transport: transport,
    );
    await tester.pump();

    expect(find.text('CPU'), findsWidgets);
    expect(find.textContaining('Score 72'), findsOneWidget);
    expect(find.textContaining('4/5'), findsOneWidget);
    expect(find.textContaining('Critical'), findsOneWidget);
  });

  testWidgets(
    'an Unknown-bottleneck verdict renders honestly, never fabricating a '
    'specific bottleneck',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/host',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              fakeVerdict(bottleneck: HostBottleneck.unknown).toJson(),
            ),
          ),
        ),
      );
      await pumpApp(
        tester,
        const Scaffold(body: HostAnalysisScreen()),
        transport: transport,
      );
      await tester.pump();

      expect(find.textContaining('Unknown'), findsOneWidget);
      expect(find.text('No bottleneck'), findsNothing);
      expect(find.text('Nothing to show'), findsOneWidget);
    },
  );

  testWidgets(
    'tapping Explain with AI fires the real request and renders a real '
    'Supported explanation',
    (tester) async {
      final transport = createFakeTransport();
      // The more specific `/explain` path is queued before its own prefix
      // `/api/v1/analysis/host` — `FakeTransport.getRaw` matches by
      // `path.startsWith(queuedPrefix)` in insertion order, so queuing
      // the shorter prefix first would make it win the `/explain`
      // request too (a real gotcha discovered writing this test, not a
      // `FakeTransport` bug: prefix matching is deliberate, for path
      // params like `/policy/:id/grant`).
      transport.queue(
        '/api/v1/analysis/host/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForAiExplanationSupported(value: fakeExplanation())
                  .toJson(),
            ),
          ),
        ),
      );
      transport.queue(
        '/api/v1/analysis/host',
        ApiOk(jsonEncode(okEnvelopeJson(fakeVerdict().toJson()))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: HostAnalysisScreen()),
        transport: transport,
      );
      await tester.pump();

      expect(find.text('Explain with AI'), findsOneWidget);
      await tester.tap(find.text('Explain with AI'));
      await tester.pump();
      await tester.pump();

      expect(find.text('The host looks CPU-bound.'), findsOneWidget);
      expect(find.text('•  CPU pressure has been sustained.'), findsOneWidget);
      expect(
        find.text('→  Consider closing background processes.'),
        findsOneWidget,
      );
      expect(find.textContaining('MEDIUM'), findsOneWidget);
      expect(find.textContaining('82%'), findsOneWidget);
    },
  );

  testWidgets('a real Unavailable gated response shows the real reason via the '
      'shared vocabulary', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/analysis/host/explain',
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
      '/api/v1/analysis/host',
      ApiOk(jsonEncode(okEnvelopeJson(fakeVerdict().toJson()))),
    );
    await pumpApp(
      tester,
      const Scaffold(body: HostAnalysisScreen()),
      transport: transport,
    );
    await tester.pump();

    await tester.tap(find.text('Explain with AI'));
    await tester.pump();
    await tester.pump();

    expect(find.text('Not available'), findsOneWidget);
    expect(find.text('AI explanation unavailable'), findsOneWidget);
  });

  testWidgets(
    'a real transport-level error shows the real failure message with a '
    'working retry',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/host/explain',
        const ApiErr(ApiFailureUnavailable('connection refused')),
      );
      transport.queue(
        '/api/v1/analysis/host',
        ApiOk(jsonEncode(okEnvelopeJson(fakeVerdict().toJson()))),
      );
      await pumpApp(
        tester,
        const Scaffold(body: HostAnalysisScreen()),
        transport: transport,
      );
      await tester.pump();

      await tester.tap(find.text('Explain with AI'));
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
        '/api/v1/analysis/host/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForAiExplanationSupported(value: fakeExplanation())
                  .toJson(),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Explain with AI'));
      await tester.pump();
      await tester.pump();

      expect(find.text('The host looks CPU-bound.'), findsOneWidget);
    },
  );
}
