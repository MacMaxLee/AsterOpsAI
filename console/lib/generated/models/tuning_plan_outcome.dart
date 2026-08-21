// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'tuning_candidate_outcome.dart';

final class TuningPlanOutcome {
  final List<TuningCandidateOutcome> candidates;
  final int planId;
  final String status;

  const TuningPlanOutcome({
    required this.candidates,
    required this.planId,
    required this.status,
  });

  static TuningPlanOutcome fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return TuningPlanOutcome(
      candidates: (map['candidates'] as List<dynamic>)
          .map((e) => TuningCandidateOutcome.fromJson(e))
          .toList(),
      planId: (map['plan_id'] as num).toInt(),
      status: map['status'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'candidates': candidates.map((e) => e.toJson()).toList(),
    'plan_id': planId,
    'status': status,
  };
}
