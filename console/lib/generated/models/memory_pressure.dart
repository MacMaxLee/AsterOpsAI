// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum MemoryPressure {
  normal._('NORMAL'),
  elevated._('ELEVATED'),
  high._('HIGH'),
  critical._('CRITICAL');

  final String wireValue;
  const MemoryPressure._(this.wireValue);

  static MemoryPressure fromJson(dynamic json) {
    final value = json as String;
    return MemoryPressure.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown MemoryPressure: $value'),
    );
  }

  String toJson() => wireValue;
}
