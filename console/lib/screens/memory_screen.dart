import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/memory_snapshot.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';
import '../widgets/metric_display.dart';
import '../widgets/metric_row.dart';
import '../widgets/pressure_badge.dart';

class MemoryScreen extends ConsumerWidget {
  const MemoryScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(memoryProvider);
    return AsyncResultView<MemorySnapshot>(
      asyncValue: async,
      builder: (context, memory) => _MemoryBody(memory: memory),
    );
  }
}

class _MemoryBody extends StatelessWidget {
  final MemorySnapshot memory;
  const _MemoryBody({required this.memory});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        PressureBadge.memory(l10n.dashboardPressureLabel, memory.pressure),
        const SizedBox(height: 12),
        MetricRow(
          label: l10n.memoryUsed,
          value: MetricValueText.uint64(memory.usedBytes, formatBytes),
        ),
        MetricRow(
          label: l10n.memoryAvailable,
          value: MetricValueText.uint64(memory.availableBytes, formatBytes),
        ),
        MetricRow(
          label: l10n.memoryTotal,
          value: MetricValueText.uint64(memory.totalBytes, formatBytes),
        ),
        MetricRow(
          label: l10n.memorySwapUsed,
          value: MetricValueText.uint64(memory.swapUsedBytes, formatBytes),
        ),
      ],
    );
  }
}
