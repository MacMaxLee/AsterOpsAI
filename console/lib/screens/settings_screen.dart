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
        for (final option in kRefreshIntervalOptions)
          RadioListTile<Duration>(
            title: Text(l10n.settingsRefreshOption(option.inSeconds)),
            value: option,
            groupValue: refreshInterval,
            onChanged: (value) {
              if (value != null) {
                ref.read(refreshIntervalProvider.notifier).setInterval(value);
              }
            },
          ),
        const Divider(height: 32),
        Text(
          l10n.settingsSectionLanguage,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        RadioListTile<Locale?>(
          title: Text(l10n.settingsLanguageSystemDefault),
          value: null,
          groupValue: locale,
          onChanged: (value) =>
              ref.read(localeProvider.notifier).setLocale(value),
        ),
        RadioListTile<Locale?>(
          title: Text(l10n.settingsLanguageEnglish),
          value: const Locale('en'),
          groupValue: locale,
          onChanged: (value) =>
              ref.read(localeProvider.notifier).setLocale(value),
        ),
        RadioListTile<Locale?>(
          title: Text(l10n.settingsLanguageChineseTraditional),
          value: const Locale.fromSubtags(
            languageCode: 'zh',
            scriptCode: 'Hant',
          ),
          groupValue: locale,
          onChanged: (value) =>
              ref.read(localeProvider.notifier).setLocale(value),
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
