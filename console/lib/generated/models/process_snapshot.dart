// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'process_info.dart';

final class ProcessSnapshot {
  final List<ProcessInfo> processes;
  final DateTime timestamp;
  final int totalCount;

  const ProcessSnapshot({
    required this.processes,
    required this.timestamp,
    required this.totalCount,
  });

  static ProcessSnapshot fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return ProcessSnapshot(
      processes: (map['processes'] as List<dynamic>)
          .map((e) => ProcessInfo.fromJson(e))
          .toList(),
      timestamp: DateTime.parse(map['timestamp'] as String),
      totalCount: (map['total_count'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'processes': processes.map((e) => e.toJson()).toList(),
    'timestamp': timestamp.toIso8601String(),
    'total_count': totalCount,
  };
}
