// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'history_stat.dart';

final class CpuHistoryPoint {
  final HistoryStat aggregateUtilizationPercent;
  final DateTime bucketEnd;
  final DateTime bucketStart;
  final HistoryStat loadAverage1m;

  const CpuHistoryPoint({
    required this.aggregateUtilizationPercent,
    required this.bucketEnd,
    required this.bucketStart,
    required this.loadAverage1m,
  });

  static CpuHistoryPoint fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return CpuHistoryPoint(
      aggregateUtilizationPercent: HistoryStat.fromJson(
        map['aggregate_utilization_percent'],
      ),
      bucketEnd: DateTime.parse(map['bucket_end'] as String),
      bucketStart: DateTime.parse(map['bucket_start'] as String),
      loadAverage1m: HistoryStat.fromJson(map['load_average_1m']),
    );
  }

  Map<String, dynamic> toJson() => {
    'aggregate_utilization_percent': aggregateUtilizationPercent.toJson(),
    'bucket_end': bucketEnd.toIso8601String(),
    'bucket_start': bucketStart.toIso8601String(),
    'load_average_1m': loadAverage1m.toJson(),
  };
}
