// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum HostBottleneck {
  none._('NONE'),
  cpu._('CPU'),
  memory._('MEMORY'),
  storageIo._('STORAGE_IO'),
  network._('NETWORK'),
  thermal._('THERMAL'),
  power._('POWER'),
  background._('BACKGROUND'),
  multiple._('MULTIPLE'),
  unknown._('UNKNOWN');

  final String wireValue;
  const HostBottleneck._(this.wireValue);

  static HostBottleneck fromJson(dynamic json) {
    final value = json as String;
    return HostBottleneck.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown HostBottleneck: $value'),
    );
  }

  String toJson() => wireValue;
}
