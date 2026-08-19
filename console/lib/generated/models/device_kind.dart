// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum DeviceKind {
  blockStorage._('BLOCK_STORAGE'),
  removableStorage._('REMOVABLE_STORAGE');

  final String wireValue;
  const DeviceKind._(this.wireValue);

  static DeviceKind fromJson(dynamic json) {
    final value = json as String;
    return DeviceKind.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown DeviceKind: $value'),
    );
  }

  String toJson() => wireValue;
}
