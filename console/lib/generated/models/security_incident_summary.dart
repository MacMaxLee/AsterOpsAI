// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class SecurityIncidentSummary {
  final DateTime? closedAt;
  final String detectorId;
  final int eventCount;
  final int id;
  final DateTime openedAt;
  final String resourceKind;
  final String resourceName;
  final String severity;
  final String status;
  final String summary;

  const SecurityIncidentSummary({
    this.closedAt,
    required this.detectorId,
    required this.eventCount,
    required this.id,
    required this.openedAt,
    required this.resourceKind,
    required this.resourceName,
    required this.severity,
    required this.status,
    required this.summary,
  });

  static SecurityIncidentSummary fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return SecurityIncidentSummary(
      closedAt: map['closed_at'] == null
          ? null
          : (DateTime.parse(map['closed_at'] as String)),
      detectorId: map['detector_id'] as String,
      eventCount: (map['event_count'] as num).toInt(),
      id: (map['id'] as num).toInt(),
      openedAt: DateTime.parse(map['opened_at'] as String),
      resourceKind: map['resource_kind'] as String,
      resourceName: map['resource_name'] as String,
      severity: map['severity'] as String,
      status: map['status'] as String,
      summary: map['summary'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'closed_at': closedAt?.toIso8601String(),
    'detector_id': detectorId,
    'event_count': eventCount,
    'id': id,
    'opened_at': openedAt.toIso8601String(),
    'resource_kind': resourceKind,
    'resource_name': resourceName,
    'severity': severity,
    'status': status,
    'summary': summary,
  };
}
