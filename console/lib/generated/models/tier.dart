// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum Tier {
  normal._('NORMAL'),
  elevated._('ELEVATED'),
  high._('HIGH'),
  critical._('CRITICAL');

  final String wireValue;
  const Tier._(this.wireValue);

  static Tier fromJson(dynamic json) {
    final value = json as String;
    return Tier.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown Tier: $value'),
    );
  }

  String toJson() => wireValue;
}
