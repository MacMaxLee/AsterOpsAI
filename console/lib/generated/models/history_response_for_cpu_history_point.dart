// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'cpu_history_point.dart';
import 'history_tier.dart';

final class HistoryResponseForCpuHistoryPoint {
  final List<CpuHistoryPoint> points;
  final DateTime requestedFrom;
  final DateTime requestedTo;
  final HistoryTier resolvedTier;
  final DateTime? truncatedFrom;

  const HistoryResponseForCpuHistoryPoint({
    required this.points,
    required this.requestedFrom,
    required this.requestedTo,
    required this.resolvedTier,
    this.truncatedFrom,
  });

  static HistoryResponseForCpuHistoryPoint fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return HistoryResponseForCpuHistoryPoint(
      points: (map['points'] as List<dynamic>)
          .map((e) => CpuHistoryPoint.fromJson(e))
          .toList(),
      requestedFrom: DateTime.parse(map['requested_from'] as String),
      requestedTo: DateTime.parse(map['requested_to'] as String),
      resolvedTier: HistoryTier.fromJson(map['resolved_tier']),
      truncatedFrom: map['truncated_from'] == null
          ? null
          : (DateTime.parse(map['truncated_from'] as String)),
    );
  }

  Map<String, dynamic> toJson() => {
    'points': points.map((e) => e.toJson()).toList(),
    'requested_from': requestedFrom.toIso8601String(),
    'requested_to': requestedTo.toIso8601String(),
    'resolved_tier': resolvedTier.toJson(),
    'truncated_from': truncatedFrom?.toIso8601String(),
  };
}
