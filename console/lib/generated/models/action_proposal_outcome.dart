// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class ActionProposalOutcome {
  final DateTime? approvalExpiresAt;
  final String? reason;
  final int rowId;
  final String status;

  const ActionProposalOutcome({
    this.approvalExpiresAt,
    this.reason,
    required this.rowId,
    required this.status,
  });

  static ActionProposalOutcome fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return ActionProposalOutcome(
      approvalExpiresAt: map['approval_expires_at'] == null
          ? null
          : (DateTime.parse(map['approval_expires_at'] as String)),
      reason: map['reason'] == null ? null : (map['reason'] as String),
      rowId: (map['row_id'] as num).toInt(),
      status: map['status'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'approval_expires_at': approvalExpiresAt?.toIso8601String(),
    'reason': reason,
    'row_id': rowId,
    'status': status,
  };
}
