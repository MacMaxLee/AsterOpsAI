import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/storage_snapshot.dart';
import '../generated/models/volume_info.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';
import '../widgets/metric_display.dart';
import '../widgets/metric_row.dart';

class StorageScreen extends ConsumerWidget {
  const StorageScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(storageProvider);
    return AsyncResultView<StorageSnapshot>(
      asyncValue: async,
      builder: (context, storage) => _StorageBody(storage: storage),
    );
  }
}

class _StorageBody extends StatelessWidget {
  final StorageSnapshot storage;
  const _StorageBody({required this.storage});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (storage.volumes.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.separated(
      padding: const EdgeInsets.all(16),
      itemCount: storage.volumes.length,
      separatorBuilder: (_, _) => const Divider(),
      itemBuilder: (context, index) =>
          _VolumeCard(volume: storage.volumes[index]),
    );
  }
}

class _VolumeCard extends StatelessWidget {
  final VolumeInfo volume;
  const _VolumeCard({required this.volume});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            volume.mountPoint,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          Text(volume.device, style: Theme.of(context).textTheme.bodySmall),
          MetricRow(
            label: l10n.storageVolumeUsed,
            value: MetricValueText.uint64(volume.capacityBytes, formatBytes),
          ),
          MetricRow(
            label: l10n.storageVolumeFree,
            value: MetricValueText.uint64(volume.freeBytes, formatBytes),
          ),
          MetricRow(
            label: l10n.storageVolumeReadRate,
            value: MetricValueText.double(
              volume.readBytesPerSec,
              formatBytesPerSec,
            ),
          ),
          MetricRow(
            label: l10n.storageVolumeWriteRate,
            value: MetricValueText.double(
              volume.writeBytesPerSec,
              formatBytesPerSec,
            ),
          ),
        ],
      ),
    );
  }
}
