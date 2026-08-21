// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class TuningCandidateOutcome {
  final String actionType;
  final String? detail;
  final String outcome;
  final int? rowId;

  const TuningCandidateOutcome({
    required this.actionType,
    this.detail,
    required this.outcome,
    this.rowId,
  });

  static TuningCandidateOutcome fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return TuningCandidateOutcome(
      actionType: map['action_type'] as String,
      detail: map['detail'] == null ? null : (map['detail'] as String),
      outcome: map['outcome'] as String,
      rowId: map['row_id'] == null ? null : ((map['row_id'] as num).toInt()),
    );
  }

  Map<String, dynamic> toJson() => {
    'action_type': actionType,
    'detail': detail,
    'outcome': outcome,
    'row_id': rowId,
  };
}
