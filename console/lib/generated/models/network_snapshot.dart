// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'network_interface_info.dart';

final class NetworkSnapshot {
  final List<NetworkInterfaceInfo> interfaces;
  final DateTime timestamp;

  const NetworkSnapshot({required this.interfaces, required this.timestamp});

  static NetworkSnapshot fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return NetworkSnapshot(
      interfaces: (map['interfaces'] as List<dynamic>)
          .map((e) => NetworkInterfaceInfo.fromJson(e))
          .toList(),
      timestamp: DateTime.parse(map['timestamp'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'interfaces': interfaces.map((e) => e.toJson()).toList(),
    'timestamp': timestamp.toIso8601String(),
  };
}
