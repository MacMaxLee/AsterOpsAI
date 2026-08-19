import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../widgets/connection_banner.dart';
import 'cpu_screen.dart';
import 'dashboard_screen.dart';
import 'devices_screen.dart';
import 'memory_screen.dart';
import 'network_screen.dart';
import 'processes_screen.dart';
import 'settings_screen.dart';
import 'storage_screen.dart';

/// Nav rail + the active screen, with the connection banner always
/// visible above the content it's currently explaining the freshness of.
class AppShell extends StatefulWidget {
  const AppShell({super.key});

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  int _selectedIndex = 0;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final destinations = [
      (l10n.navDashboard, Icons.dashboard_outlined, const DashboardScreen()),
      (l10n.navCpu, Icons.memory_outlined, const CpuScreen()),
      (l10n.navMemory, Icons.sd_storage_outlined, const MemoryScreen()),
      (l10n.navStorage, Icons.storage_outlined, const StorageScreen()),
      (l10n.navNetwork, Icons.lan_outlined, const NetworkScreen()),
      (l10n.navProcesses, Icons.list_alt_outlined, const ProcessesScreen()),
      (l10n.navDevices, Icons.devices_other_outlined, const DevicesScreen()),
      (l10n.navSettings, Icons.settings_outlined, const SettingsScreen()),
    ];

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: _selectedIndex,
            labelType: NavigationRailLabelType.all,
            onDestinationSelected: (index) =>
                setState(() => _selectedIndex = index),
            destinations: [
              for (final d in destinations)
                NavigationRailDestination(icon: Icon(d.$2), label: Text(d.$1)),
            ],
          ),
          const VerticalDivider(width: 1, thickness: 1),
          Expanded(
            child: Column(
              children: [
                const ConnectionBanner(),
                Expanded(
                  child: Semantics(
                    label: destinations[_selectedIndex].$1,
                    child: destinations[_selectedIndex].$3,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
