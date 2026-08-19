import 'package:console/generated/models/capability.dart';
import 'package:console/generated/models/metric_value_for_double.dart';
import 'package:console/l10n/app_localizations.dart';
import 'package:console/widgets/metric_display.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> _pump(WidgetTester tester, Widget child) async {
  await tester.pumpWidget(
    MaterialApp(
      supportedLocales: AppLocalizations.supportedLocales,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      home: Scaffold(body: Center(child: child)),
    ),
  );
}

void main() {
  testWidgets('SUPPORTED renders the formatted value, no badge', (
    tester,
  ) async {
    await _pump(
      tester,
      MetricValueText.double(
        const MetricValueForDoubleSupported(value: 42),
        (v) => '${v.toStringAsFixed(0)}%',
      ),
    );
    expect(find.text('42%'), findsOneWidget);
    expect(find.byIcon(Icons.hourglass_empty), findsNothing);
  });

  testWidgets('SAMPLE_GAP never falls back to a bare value or dash', (
    tester,
  ) async {
    await _pump(
      tester,
      MetricValueText.double(
        const MetricValueForDoubleSampleGap(reason: 'gap > 2x interval'),
        (v) => '${v.toStringAsFixed(0)}%',
      ),
    );
    expect(find.byIcon(Icons.hourglass_empty), findsOneWidget);
    expect(find.text('—'), findsNothing);
    expect(find.text('0%'), findsNothing);
  });

  testWidgets('COUNTER_RESET is visually distinct from SAMPLE_GAP', (
    tester,
  ) async {
    await _pump(
      tester,
      MetricValueText.double(
        const MetricValueForDoubleCounterReset(reason: 'nic reinit'),
        (v) => '${v.toStringAsFixed(0)}%',
      ),
    );
    expect(find.byIcon(Icons.restart_alt), findsOneWidget);
    expect(find.byIcon(Icons.hourglass_empty), findsNothing);
  });

  testWidgets('UNAVAILABLE never renders a fabricated zero', (tester) async {
    await _pump(
      tester,
      MetricValueText.double(
        const MetricValueForDoubleUnavailable(reason: 'not supported here'),
        (v) => '${v.toStringAsFixed(0)}%',
      ),
    );
    expect(find.byIcon(Icons.remove_circle_outline), findsOneWidget);
    expect(find.text('0%'), findsNothing);
  });

  testWidgets('Capability LIMITED is distinct from UNAVAILABLE', (
    tester,
  ) async {
    await _pump(
      tester,
      MetricValueText.capability(
        const CapabilityLimited(reason: 'reduced sampling rate'),
      ),
    );
    expect(find.byIcon(Icons.info_outline), findsOneWidget);
  });

  testWidgets('Capability PERMISSION_REQUIRED is distinct from UNAVAILABLE', (
    tester,
  ) async {
    await _pump(
      tester,
      MetricValueText.capability(
        const CapabilityPermissionRequired(reason: 'needs elevated access'),
      ),
    );
    expect(find.byIcon(Icons.lock_outline), findsOneWidget);
  });
}
