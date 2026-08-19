// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum CpuPressure {
  normal._('NORMAL'),
  elevated._('ELEVATED'),
  high._('HIGH'),
  critical._('CRITICAL');

  final String wireValue;
  const CpuPressure._(this.wireValue);

  static CpuPressure fromJson(dynamic json) {
    final value = json as String;
    return CpuPressure.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown CpuPressure: $value'),
    );
  }

  String toJson() => wireValue;
}
