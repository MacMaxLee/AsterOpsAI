// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'device_kind.dart';

final class DeviceInfo {
  final String identifier;
  final DeviceKind kind;
  final String name;
  final bool removable;
  final bool trusted;

  const DeviceInfo({
    required this.identifier,
    required this.kind,
    required this.name,
    required this.removable,
    required this.trusted,
  });

  static DeviceInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return DeviceInfo(
      identifier: map['identifier'] as String,
      kind: DeviceKind.fromJson(map['kind']),
      name: map['name'] as String,
      removable: map['removable'] as bool,
      trusted: map['trusted'] as bool,
    );
  }

  Map<String, dynamic> toJson() => {
    'identifier': identifier,
    'kind': kind.toJson(),
    'name': name,
    'removable': removable,
    'trusted': trusted,
  };
}
