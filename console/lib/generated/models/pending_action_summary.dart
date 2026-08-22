// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class PendingActionSummary {
  final String actionType;
  final DateTime? approvalExpiresAt;
  final DateTime createdAt;
  final int id;
  final String requestedBy;
  final String resourceKind;
  final String resourceName;
  final String riskClassification;
  final String status;

  const PendingActionSummary({
    required this.actionType,
    this.approvalExpiresAt,
    required this.createdAt,
    required this.id,
    required this.requestedBy,
    required this.resourceKind,
    required this.resourceName,
    required this.riskClassification,
    required this.status,
  });

  static PendingActionSummary fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return PendingActionSummary(
      actionType: map['action_type'] as String,
      approvalExpiresAt: map['approval_expires_at'] == null
          ? null
          : (DateTime.parse(map['approval_expires_at'] as String)),
      createdAt: DateTime.parse(map['created_at'] as String),
      id: (map['id'] as num).toInt(),
      requestedBy: map['requested_by'] as String,
      resourceKind: map['resource_kind'] as String,
      resourceName: map['resource_name'] as String,
      riskClassification: map['risk_classification'] as String,
      status: map['status'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'action_type': actionType,
    'approval_expires_at': approvalExpiresAt?.toIso8601String(),
    'created_at': createdAt.toIso8601String(),
    'id': id,
    'requested_by': requestedBy,
    'resource_kind': resourceKind,
    'resource_name': resourceName,
    'risk_classification': riskClassification,
    'status': status,
  };
}
