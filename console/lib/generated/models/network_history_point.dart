// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'history_stat.dart';

final class NetworkHistoryPoint {
  final DateTime bucketEnd;
  final DateTime bucketStart;
  final HistoryStat rxBytesPerSec;
  final HistoryStat txBytesPerSec;

  const NetworkHistoryPoint({
    required this.bucketEnd,
    required this.bucketStart,
    required this.rxBytesPerSec,
    required this.txBytesPerSec,
  });

  static NetworkHistoryPoint fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return NetworkHistoryPoint(
      bucketEnd: DateTime.parse(map['bucket_end'] as String),
      bucketStart: DateTime.parse(map['bucket_start'] as String),
      rxBytesPerSec: HistoryStat.fromJson(map['rx_bytes_per_sec']),
      txBytesPerSec: HistoryStat.fromJson(map['tx_bytes_per_sec']),
    );
  }

  Map<String, dynamic> toJson() => {
    'bucket_end': bucketEnd.toIso8601String(),
    'bucket_start': bucketStart.toIso8601String(),
    'rx_bytes_per_sec': rxBytesPerSec.toJson(),
    'tx_bytes_per_sec': txBytesPerSec.toJson(),
  };
}
