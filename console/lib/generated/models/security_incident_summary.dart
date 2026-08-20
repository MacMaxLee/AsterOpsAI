// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class SecurityIncidentSummary {
  final DateTime? closedAt;
  final int eventCount;
  final int id;
  final DateTime openedAt;
  final String severity;
  final String status;
  final String summary;

  const SecurityIncidentSummary({
    this.closedAt,
    required this.eventCount,
    required this.id,
    required this.openedAt,
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
      eventCount: (map['event_count'] as num).toInt(),
      id: (map['id'] as num).toInt(),
      openedAt: DateTime.parse(map['opened_at'] as String),
      severity: map['severity'] as String,
      status: map['status'] as String,
      summary: map['summary'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'closed_at': closedAt?.toIso8601String(),
    'event_count': eventCount,
    'id': id,
    'opened_at': openedAt.toIso8601String(),
    'severity': severity,
    'status': status,
    'summary': summary,
  };
}
