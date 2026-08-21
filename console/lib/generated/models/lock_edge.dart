// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class LockEdge {
  final int blockedPid;
  final String? blockedQuery;
  final int blockingPid;
  final String? blockingQuery;
  final String lockType;

  const LockEdge({
    required this.blockedPid,
    this.blockedQuery,
    required this.blockingPid,
    this.blockingQuery,
    required this.lockType,
  });

  static LockEdge fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return LockEdge(
      blockedPid: (map['blocked_pid'] as num).toInt(),
      blockedQuery: map['blocked_query'] == null
          ? null
          : (map['blocked_query'] as String),
      blockingPid: (map['blocking_pid'] as num).toInt(),
      blockingQuery: map['blocking_query'] == null
          ? null
          : (map['blocking_query'] as String),
      lockType: map['lock_type'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'blocked_pid': blockedPid,
    'blocked_query': blockedQuery,
    'blocking_pid': blockingPid,
    'blocking_query': blockingQuery,
    'lock_type': lockType,
  };
}
