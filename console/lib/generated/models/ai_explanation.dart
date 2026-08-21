// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'observation.dart';
import 'recommendation.dart';
import 'risk_level.dart';

final class AiExplanation {
  final double confidence;
  final List<Observation> observations;
  final List<Recommendation> recommendations;
  final RiskLevel risk;
  final String summary;

  const AiExplanation({
    required this.confidence,
    required this.observations,
    required this.recommendations,
    required this.risk,
    required this.summary,
  });

  static AiExplanation fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return AiExplanation(
      confidence: (map['confidence'] as num).toDouble(),
      observations: (map['observations'] as List<dynamic>)
          .map((e) => Observation.fromJson(e))
          .toList(),
      recommendations: (map['recommendations'] as List<dynamic>)
          .map((e) => Recommendation.fromJson(e))
          .toList(),
      risk: RiskLevel.fromJson(map['risk']),
      summary: map['summary'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'confidence': confidence,
    'observations': observations.map((e) => e.toJson()).toList(),
    'recommendations': recommendations.map((e) => e.toJson()).toList(),
    'risk': risk.toJson(),
    'summary': summary,
  };
}
