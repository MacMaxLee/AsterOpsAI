// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class DeadlockInfo {
  final int deadlocks;
  final DateTime? statsReset;

  const DeadlockInfo({required this.deadlocks, this.statsReset});

  static DeadlockInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return DeadlockInfo(
      deadlocks: (map['deadlocks'] as num).toInt(),
      statsReset: map['stats_reset'] == null
          ? null
          : (DateTime.parse(map['stats_reset'] as String)),
    );
  }

  Map<String, dynamic> toJson() => {
    'deadlocks': deadlocks,
    'stats_reset': statsReset?.toIso8601String(),
  };
}
