import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/device_info.dart';
import '../generated/models/device_snapshot.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../widgets/async_result_view.dart';

class DevicesScreen extends ConsumerWidget {
  const DevicesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(devicesProvider);
    return AsyncResultView<DeviceSnapshot>(
      asyncValue: async,
      builder: (context, snapshot) => _DevicesBody(snapshot: snapshot),
    );
  }
}

class _DevicesBody extends StatelessWidget {
  final DeviceSnapshot snapshot;
  const _DevicesBody({required this.snapshot});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (snapshot.devices.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: snapshot.devices.length,
      itemBuilder: (context, index) =>
          _DeviceRow(device: snapshot.devices[index]),
    );
  }
}

class _DeviceRow extends StatelessWidget {
  final DeviceInfo device;
  const _DeviceRow({required this.device});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      leading: Icon(device.trusted ? Icons.verified_user : Icons.help_outline),
      title: Text(device.name),
      subtitle: Text(
        '${l10n.deviceColumnKind}: ${device.kind.wireValue}  •  ${l10n.deviceColumnTrusted}: ${device.trusted}',
      ),
    );
  }
}
