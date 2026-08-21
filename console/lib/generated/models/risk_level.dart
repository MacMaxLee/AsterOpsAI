// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum RiskLevel {
  low._('LOW'),
  medium._('MEDIUM'),
  high._('HIGH'),
  critical._('CRITICAL');

  final String wireValue;
  const RiskLevel._(this.wireValue);

  static RiskLevel fromJson(dynamic json) {
    final value = json as String;
    return RiskLevel.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown RiskLevel: $value'),
    );
  }

  String toJson() => wireValue;
}
