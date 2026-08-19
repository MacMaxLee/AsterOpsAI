// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class HistoryStat {
  final double? avg;
  final double? max;
  final double? min;
  final int supportedSampleCount;
  final int totalSampleCount;

  const HistoryStat({
    this.avg,
    this.max,
    this.min,
    required this.supportedSampleCount,
    required this.totalSampleCount,
  });

  static HistoryStat fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return HistoryStat(
      avg: map['avg'] == null ? null : ((map['avg'] as num).toDouble()),
      max: map['max'] == null ? null : ((map['max'] as num).toDouble()),
      min: map['min'] == null ? null : ((map['min'] as num).toDouble()),
      supportedSampleCount: (map['supported_sample_count'] as num).toInt(),
      totalSampleCount: (map['total_sample_count'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'avg': avg,
    'max': max,
    'min': min,
    'supported_sample_count': supportedSampleCount,
    'total_sample_count': totalSampleCount,
  };
}
