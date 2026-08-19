import 'package:flutter/material.dart';

import '../api/api_version.dart';
import '../l10n/app_localizations.dart';

/// Requirement 5: an incompatible `api_version` blocks the whole console
/// rather than letting individual screens fail per-request. This replaces
/// the app shell entirely — no nav, no data screens reachable behind it.
class UpgradeRequiredScreen extends StatelessWidget {
  final String coreVersion;
  const UpgradeRequiredScreen({required this.coreVersion, super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.system_update, size: 48),
              const SizedBox(height: 16),
              Text(
                l10n.upgradeRequiredTitle,
                style: Theme.of(context).textTheme.headlineSmall,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                l10n.upgradeRequiredBody(kSupportedApiVersion, coreVersion),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
