import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/process_info.dart';
import '../generated/models/process_snapshot.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';
import '../widgets/metric_display.dart';

class ProcessesScreen extends ConsumerWidget {
  const ProcessesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(processesProvider);
    return AsyncResultView<ProcessSnapshot>(
      asyncValue: async,
      builder: (context, snapshot) => _ProcessesBody(snapshot: snapshot),
    );
  }
}

class _ProcessesBody extends StatelessWidget {
  final ProcessSnapshot snapshot;
  const _ProcessesBody({required this.snapshot});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (snapshot.processes.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(8),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(l10n.processesTotalCount(snapshot.totalCount)),
          ),
        ),
        Expanded(
          child: ListView.builder(
            itemCount: snapshot.processes.length,
            itemBuilder: (context, index) =>
                _ProcessRow(process: snapshot.processes[index]),
          ),
        ),
      ],
    );
  }
}

class _ProcessRow extends StatelessWidget {
  final ProcessInfo process;
  const _ProcessRow({required this.process});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text(process.comm),
      subtitle: Text(
        '${l10n.processColumnPid}: ${process.pid}  •  ${l10n.processColumnCategory}: ${process.category.wireValue}',
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          MetricValueText.double(
            process.cpuPercent,
            (v) => '${v.toStringAsFixed(1)}%',
          ),
          const SizedBox(width: 16),
          MetricValueText.uint64(process.rssBytes, formatBytes),
        ],
      ),
    );
  }
}
