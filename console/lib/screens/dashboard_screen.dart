import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/system_status_response.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';
import '../widgets/metric_display.dart';
import '../widgets/metric_row.dart';
import '../widgets/pressure_badge.dart';

class DashboardScreen extends ConsumerWidget {
  const DashboardScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(systemStatusProvider);
    final cpu = ref.watch(cpuProvider);
    final memory = ref.watch(memoryProvider);
    final l10n = AppLocalizations.of(context)!;

    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        AsyncResultView<SystemStatusResponse>(
          asyncValue: status,
          builder: (context, s) => Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(l10n.dashboardUptimeLabel(formatDuration(s.uptimeSeconds))),
              const SizedBox(height: 8),
              PressureBadge.cpu(l10n.dashboardSectionCpu, s.cpuPressure),
              const SizedBox(height: 4),
              PressureBadge.memory(
                l10n.dashboardSectionMemory,
                s.memoryPressure,
              ),
            ],
          ),
        ),
        const Divider(height: 32),
        Text(
          l10n.dashboardSectionCpu,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        AsyncResultView(
          asyncValue: cpu,
          builder: (context, snapshot) => MetricRow(
            label: l10n.cpuAggregateUtilization,
            value: MetricValueText.double(
              snapshot.aggregateUtilizationPercent,
              (v) => '${v.toStringAsFixed(1)}%',
            ),
          ),
        ),
        const SizedBox(height: 16),
        Text(
          l10n.dashboardSectionMemory,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        AsyncResultView(
          asyncValue: memory,
          builder: (context, snapshot) => MetricRow(
            label: l10n.memoryUsed,
            value: MetricValueText.uint64(snapshot.usedBytes, formatBytes),
          ),
        ),
      ],
    );
  }
}
