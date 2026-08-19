import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../providers/connection_status.dart';
import '../providers/settings_provider.dart';

class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final refreshInterval = ref.watch(refreshIntervalProvider);
    final locale = ref.watch(localeProvider);
    final connection = ref.watch(connectionStatusProvider);

    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Text(
          l10n.settingsSectionRefresh,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        // RadioGroup gives the whole section WAI-ARIA-pattern keyboard
        // navigation (arrow keys move selection, tab enters/exits the
        // group as one stop) for free — directly serves U3 requirement 7.
        RadioGroup<Duration>(
          groupValue: refreshInterval,
          onChanged: (value) {
            if (value != null) {
              ref.read(refreshIntervalProvider.notifier).setInterval(value);
            }
          },
          child: Column(
            children: [
              for (final option in kRefreshIntervalOptions)
                RadioListTile<Duration>(
                  title: Text(l10n.settingsRefreshOption(option.inSeconds)),
                  value: option,
                ),
            ],
          ),
        ),
        const Divider(height: 32),
        Text(
          l10n.settingsSectionLanguage,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        RadioGroup<Locale?>(
          groupValue: locale,
          onChanged: (value) =>
              ref.read(localeProvider.notifier).setLocale(value),
          child: Column(
            children: [
              RadioListTile<Locale?>(
                title: Text(l10n.settingsLanguageSystemDefault),
                value: null,
              ),
              RadioListTile<Locale?>(
                title: Text(l10n.settingsLanguageEnglish),
                value: const Locale('en'),
              ),
              RadioListTile<Locale?>(
                title: Text(l10n.settingsLanguageChineseTraditional),
                value: const Locale.fromSubtags(
                  languageCode: 'zh',
                  scriptCode: 'Hant',
                ),
              ),
            ],
          ),
        ),
        const Divider(height: 32),
        Text(
          l10n.settingsSectionAbout,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        if (connection is ConnectionConnected)
          Text(l10n.settingsCoreVersion(connection.health.version)),
      ],
    );
  }
}
