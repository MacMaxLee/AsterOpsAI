// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum HistoryTier {
  raw._('RAW'),
  hourly._('HOURLY'),
  daily._('DAILY');

  final String wireValue;
  const HistoryTier._(this.wireValue);

  static HistoryTier fromJson(dynamic json) {
    final value = json as String;
    return HistoryTier.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown HistoryTier: $value'),
    );
  }

  String toJson() => wireValue;
}
