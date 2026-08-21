// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class ResumableActionSummary {
  final String actionType;
  final DateTime executedAt;
  final int rowId;

  const ResumableActionSummary({
    required this.actionType,
    required this.executedAt,
    required this.rowId,
  });

  static ResumableActionSummary fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return ResumableActionSummary(
      actionType: map['action_type'] as String,
      executedAt: DateTime.parse(map['executed_at'] as String),
      rowId: (map['row_id'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'action_type': actionType,
    'executed_at': executedAt.toIso8601String(),
    'row_id': rowId,
  };
}
