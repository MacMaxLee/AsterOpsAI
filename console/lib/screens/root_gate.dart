import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/connection_status.dart';
import 'app_shell.dart';
import 'upgrade_required_screen.dart';

/// The one place `ConnectionVersionMismatch` is checked — before the shell,
/// nav, or any data screen ever mounts (U3 requirement 5). Every other
/// connection state is handled per-screen via the banner + per-request
/// failure rendering; this is the sole exception that blocks the whole app.
class RootGate extends ConsumerWidget {
  const RootGate({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(connectionStatusProvider);
    if (status is ConnectionVersionMismatch) {
      return UpgradeRequiredScreen(coreVersion: status.coreVersion);
    }
    return const AppShell();
  }
}
