// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'evidence.dart';
import 'root_cause.dart';

final class Hypothesis {
  final RootCause cause;
  final double confidence;
  final List<Evidence> evidence;

  const Hypothesis({
    required this.cause,
    required this.confidence,
    required this.evidence,
  });

  static Hypothesis fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return Hypothesis(
      cause: RootCause.fromJson(map['cause']),
      confidence: (map['confidence'] as num).toDouble(),
      evidence: (map['evidence'] as List<dynamic>)
          .map((e) => Evidence.fromJson(e))
          .toList(),
    );
  }

  Map<String, dynamic> toJson() => {
    'cause': cause.toJson(),
    'confidence': confidence,
    'evidence': evidence.map((e) => e.toJson()).toList(),
  };
}
