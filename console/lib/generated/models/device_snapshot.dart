// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'device_info.dart';

final class DeviceSnapshot {
  final List<DeviceInfo> devices;
  final DateTime timestamp;

  const DeviceSnapshot({required this.devices, required this.timestamp});

  static DeviceSnapshot fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return DeviceSnapshot(
      devices: (map['devices'] as List<dynamic>)
          .map((e) => DeviceInfo.fromJson(e))
          .toList(),
      timestamp: DateTime.parse(map['timestamp'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'devices': devices.map((e) => e.toJson()).toList(),
    'timestamp': timestamp.toIso8601String(),
  };
}
