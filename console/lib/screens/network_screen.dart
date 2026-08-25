import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/network_interface_info.dart';
import '../generated/models/network_snapshot.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';
import '../widgets/metric_display.dart';
import '../widgets/metric_row.dart';

class NetworkScreen extends ConsumerWidget {
  const NetworkScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(networkProvider);
    return AsyncResultView<NetworkSnapshot>(
      asyncValue: async,
      builder: (context, network) => _NetworkBody(network: network),
    );
  }
}

class _NetworkBody extends StatelessWidget {
  final NetworkSnapshot network;
  const _NetworkBody({required this.network});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (network.interfaces.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.separated(
      padding: const EdgeInsets.all(16),
      itemCount: network.interfaces.length,
      separatorBuilder: (_, __) => const Divider(),
      itemBuilder: (context, index) =>
          _InterfaceCard(iface: network.interfaces[index]),
    );
  }
}

class _InterfaceCard extends StatelessWidget {
  final NetworkInterfaceInfo iface;
  const _InterfaceCard({required this.iface});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(iface.name, style: Theme.of(context).textTheme.titleMedium),
          MetricRow(
            label: l10n.networkRxRate,
            value: MetricValueText.double(
              iface.rxBytesPerSec,
              formatBytesPerSec,
            ),
          ),
          MetricRow(
            label: l10n.networkTxRate,
            value: MetricValueText.double(
              iface.txBytesPerSec,
              formatBytesPerSec,
            ),
          ),
        ],
      ),
    );
  }
}
