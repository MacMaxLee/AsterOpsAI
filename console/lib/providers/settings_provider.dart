import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Overridden with a real instance in main() after `SharedPreferences.
/// getInstance()` resolves — Riverpod's standard pattern for a dependency
/// that's only available after an async bootstrap step.
final sharedPreferencesProvider = Provider<SharedPreferences>((ref) {
  throw UnimplementedError(
    'sharedPreferencesProvider must be overridden in main()',
  );
});

const kDefaultRefreshInterval = Duration(seconds: 2);
const kRefreshIntervalOptions = [
  Duration(seconds: 1),
  Duration(seconds: 2),
  Duration(seconds: 5),
  Duration(seconds: 10),
];

const _refreshIntervalKey = 'refresh_interval_seconds';

final class RefreshIntervalNotifier extends StateNotifier<Duration> {
  final SharedPreferences _prefs;

  RefreshIntervalNotifier(this._prefs)
    : super(
        Duration(
          seconds:
              _prefs.getInt(_refreshIntervalKey) ??
              kDefaultRefreshInterval.inSeconds,
        ),
      );

  void setInterval(Duration interval) {
    state = interval;
    _prefs.setInt(_refreshIntervalKey, interval.inSeconds);
  }
}

final refreshIntervalProvider =
    StateNotifierProvider<RefreshIntervalNotifier, Duration>((ref) {
      return RefreshIntervalNotifier(ref.watch(sharedPreferencesProvider));
    });

const _localeKey = 'locale';

/// `null` means "follow the system locale."
final class LocaleNotifier extends StateNotifier<Locale?> {
  final SharedPreferences _prefs;

  LocaleNotifier(this._prefs) : super(_load(_prefs));

  static Locale? _load(SharedPreferences prefs) {
    final saved = prefs.getString(_localeKey);
    if (saved == null) return null;
    final parts = saved.split('_');
    return parts.length == 1
        ? Locale(parts[0])
        : Locale.fromSubtags(languageCode: parts[0], scriptCode: parts[1]);
  }

  void setLocale(Locale? locale) {
    state = locale;
    if (locale == null) {
      _prefs.remove(_localeKey);
      return;
    }
    final tag = locale.scriptCode == null
        ? locale.languageCode
        : '${locale.languageCode}_${locale.scriptCode}';
    _prefs.setString(_localeKey, tag);
  }
}

final localeProvider = StateNotifierProvider<LocaleNotifier, Locale?>((ref) {
  return LocaleNotifier(ref.watch(sharedPreferencesProvider));
});
