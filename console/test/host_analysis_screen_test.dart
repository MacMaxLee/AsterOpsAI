import 'dart:convert';

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
}
