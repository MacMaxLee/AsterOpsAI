// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'metric_claim.dart';

final class Observation {
  final List<MetricClaim> metrics;
  final String text;

  const Observation({required this.metrics, required this.text});

  static Observation fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return Observation(
      metrics: (map['metrics'] as List<dynamic>)
          .map((e) => MetricClaim.fromJson(e))
          .toList(),
      text: map['text'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'metrics': metrics.map((e) => e.toJson()).toList(),
    'text': text,
  };
}
