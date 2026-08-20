// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class Evidence {
  final String metric;
  final double observed;
  final double threshold;
  final String? unit;
  final DateTime windowEnd;
  final DateTime windowStart;

  const Evidence({
    required this.metric,
    required this.observed,
    required this.threshold,
    this.unit,
    required this.windowEnd,
    required this.windowStart,
  });

  static Evidence fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return Evidence(
      metric: map['metric'] as String,
      observed: (map['observed'] as num).toDouble(),
      threshold: (map['threshold'] as num).toDouble(),
      unit: map['unit'] == null ? null : (map['unit'] as String),
      windowEnd: DateTime.parse(map['window_end'] as String),
      windowStart: DateTime.parse(map['window_start'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'metric': metric,
    'observed': observed,
    'threshold': threshold,
    'unit': unit,
    'window_end': windowEnd.toIso8601String(),
    'window_start': windowStart.toIso8601String(),
  };
}
