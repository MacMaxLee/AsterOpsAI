// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum HostDomain {
  cpu._('CPU'),
  memory._('MEMORY'),
  storageIo._('STORAGE_IO'),
  network._('NETWORK');

  final String wireValue;
  const HostDomain._(this.wireValue);

  static HostDomain fromJson(dynamic json) {
    final value = json as String;
    return HostDomain.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown HostDomain: $value'),
    );
  }

  String toJson() => wireValue;
}
