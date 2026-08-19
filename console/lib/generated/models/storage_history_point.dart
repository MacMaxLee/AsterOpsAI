// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'history_stat.dart';

final class StorageHistoryPoint {
  final DateTime bucketEnd;
  final DateTime bucketStart;
  final HistoryStat readBytesPerSec;
  final HistoryStat writeBytesPerSec;

  const StorageHistoryPoint({
    required this.bucketEnd,
    required this.bucketStart,
    required this.readBytesPerSec,
    required this.writeBytesPerSec,
  });

  static StorageHistoryPoint fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return StorageHistoryPoint(
      bucketEnd: DateTime.parse(map['bucket_end'] as String),
      bucketStart: DateTime.parse(map['bucket_start'] as String),
      readBytesPerSec: HistoryStat.fromJson(map['read_bytes_per_sec']),
      writeBytesPerSec: HistoryStat.fromJson(map['write_bytes_per_sec']),
    );
  }

  Map<String, dynamic> toJson() => {
    'bucket_end': bucketEnd.toIso8601String(),
    'bucket_start': bucketStart.toIso8601String(),
    'read_bytes_per_sec': readBytesPerSec.toJson(),
    'write_bytes_per_sec': writeBytesPerSec.toJson(),
  };
}
