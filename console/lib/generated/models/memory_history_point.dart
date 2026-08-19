// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'history_stat.dart';

final class MemoryHistoryPoint {
  final DateTime bucketEnd;
  final DateTime bucketStart;
  final HistoryStat usedBytes;

  const MemoryHistoryPoint({
    required this.bucketEnd,
    required this.bucketStart,
    required this.usedBytes,
  });

  static MemoryHistoryPoint fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return MemoryHistoryPoint(
      bucketEnd: DateTime.parse(map['bucket_end'] as String),
      bucketStart: DateTime.parse(map['bucket_start'] as String),
      usedBytes: HistoryStat.fromJson(map['used_bytes']),
    );
  }

  Map<String, dynamic> toJson() => {
    'bucket_end': bucketEnd.toIso8601String(),
    'bucket_start': bucketStart.toIso8601String(),
    'used_bytes': usedBytes.toJson(),
  };
}
