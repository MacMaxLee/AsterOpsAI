import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/cpu_snapshot.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';
import '../widgets/metric_display.dart';
import '../widgets/metric_row.dart';
import '../widgets/pressure_badge.dart';

class CpuScreen extends ConsumerWidget {
  const CpuScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(cpuProvider);
    return AsyncResultView<CpuSnapshot>(
      asyncValue: async,
      builder: (context, cpu) => _CpuBody(cpu: cpu),
    );
  }
}

class _CpuBody extends StatelessWidget {
  final CpuSnapshot cpu;
  const _CpuBody({required this.cpu});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        PressureBadge.cpu(l10n.dashboardPressureLabel, cpu.pressure),
        const SizedBox(height: 12),
        MetricRow(
          label: l10n.cpuAggregateUtilization,
          value: MetricValueText.double(
            cpu.aggregateUtilizationPercent,
            (v) => '${v.toStringAsFixed(1)}%',
          ),
        ),
        MetricRow(
          label: l10n.cpuLoadAverage1m,
          value: MetricValueText.double(
            cpu.loadAverage1m,
            (v) => v.toStringAsFixed(2),
          ),
        ),
        MetricRow(
          label: l10n.cpuLoadAverage5m,
          value: MetricValueText.double(
            cpu.loadAverage5m,
            (v) => v.toStringAsFixed(2),
          ),
        ),
        MetricRow(
          label: l10n.cpuLoadAverage15m,
          value: MetricValueText.double(
            cpu.loadAverage15m,
            (v) => v.toStringAsFixed(2),
          ),
        ),
        MetricRow(
          label: l10n.cpuLogicalCores,
          value: Text('${cpu.logicalCoreCount}'),
        ),
        const SizedBox(height: 16),
        Text(
          l10n.cpuPerCoreUtilization,
          style: Theme.of(context).textTheme.titleSmall,
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 12,
          runSpacing: 8,
          children: [
            for (var i = 0; i < cpu.perCoreUtilizationPercent.length; i++)
              Chip(
                label: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text('core $i: '),
                    MetricValueText.double(
                      cpu.perCoreUtilizationPercent[i],
                      (v) => '${v.toStringAsFixed(0)}%',
                    ),
                  ],
                ),
              ),
          ],
        ),
      ],
    );
  }
}
