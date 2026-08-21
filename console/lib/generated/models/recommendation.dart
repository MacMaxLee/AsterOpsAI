// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'metric_claim.dart';

final class Recommendation {
  final int? candidateRef;
  final List<MetricClaim> metrics;
  final String text;

  const Recommendation({
    this.candidateRef,
    required this.metrics,
    required this.text,
  });

  static Recommendation fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return Recommendation(
      candidateRef: map['candidate_ref'] == null
          ? null
          : ((map['candidate_ref'] as num).toInt()),
      metrics: (map['metrics'] as List<dynamic>)
          .map((e) => MetricClaim.fromJson(e))
          .toList(),
      text: map['text'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'candidate_ref': candidateRef,
    'metrics': metrics.map((e) => e.toJson()).toList(),
    'text': text,
  };
}
