// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'hypothesis.dart';
import 'ruled_out.dart';

final class CorrelationResult {
  final List<Hypothesis> ranked;
  final List<RuledOut> ruledOut;
  final DateTime windowEnd;
  final DateTime windowStart;

  const CorrelationResult({
    required this.ranked,
    required this.ruledOut,
    required this.windowEnd,
    required this.windowStart,
  });

  static CorrelationResult fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return CorrelationResult(
      ranked: (map['ranked'] as List<dynamic>)
          .map((e) => Hypothesis.fromJson(e))
          .toList(),
      ruledOut: (map['ruled_out'] as List<dynamic>)
          .map((e) => RuledOut.fromJson(e))
          .toList(),
      windowEnd: DateTime.parse(map['window_end'] as String),
      windowStart: DateTime.parse(map['window_start'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'ranked': ranked.map((e) => e.toJson()).toList(),
    'ruled_out': ruledOut.map((e) => e.toJson()).toList(),
    'window_end': windowEnd.toIso8601String(),
    'window_start': windowStart.toIso8601String(),
  };
}
