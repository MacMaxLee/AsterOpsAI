// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'volume_info.dart';

final class StorageSnapshot {
  final DateTime timestamp;
  final List<VolumeInfo> volumes;

  const StorageSnapshot({required this.timestamp, required this.volumes});

  static StorageSnapshot fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return StorageSnapshot(
      timestamp: DateTime.parse(map['timestamp'] as String),
      volumes: (map['volumes'] as List<dynamic>)
          .map((e) => VolumeInfo.fromJson(e))
          .toList(),
    );
  }

  Map<String, dynamic> toJson() => {
    'timestamp': timestamp.toIso8601String(),
    'volumes': volumes.map((e) => e.toJson()).toList(),
  };
}
