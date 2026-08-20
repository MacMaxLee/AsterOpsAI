// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class TuningPlanSummary {
  final String candidatesJson;
  final DateTime? completedAt;
  final DateTime createdAt;
  final int id;
  final String mode;
  final String profile;
  final String status;
  final String targetIdentityJson;

  const TuningPlanSummary({
    required this.candidatesJson,
    this.completedAt,
    required this.createdAt,
    required this.id,
    required this.mode,
    required this.profile,
    required this.status,
    required this.targetIdentityJson,
  });

  static TuningPlanSummary fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return TuningPlanSummary(
      candidatesJson: map['candidates_json'] as String,
      completedAt: map['completed_at'] == null
          ? null
          : (DateTime.parse(map['completed_at'] as String)),
      createdAt: DateTime.parse(map['created_at'] as String),
      id: (map['id'] as num).toInt(),
      mode: map['mode'] as String,
      profile: map['profile'] as String,
      status: map['status'] as String,
      targetIdentityJson: map['target_identity_json'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'candidates_json': candidatesJson,
    'completed_at': completedAt?.toIso8601String(),
    'created_at': createdAt.toIso8601String(),
    'id': id,
    'mode': mode,
    'profile': profile,
    'status': status,
    'target_identity_json': targetIdentityJson,
  };
}
