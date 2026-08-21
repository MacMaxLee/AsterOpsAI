// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class QueryStat {
  final int calls;
  final double meanExecTimeMs;
  final String normalizedQuery;
  final String queryFingerprint;
  final int rows;
  final double totalExecTimeMs;

  const QueryStat({
    required this.calls,
    required this.meanExecTimeMs,
    required this.normalizedQuery,
    required this.queryFingerprint,
    required this.rows,
    required this.totalExecTimeMs,
  });

  static QueryStat fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return QueryStat(
      calls: (map['calls'] as num).toInt(),
      meanExecTimeMs: (map['mean_exec_time_ms'] as num).toDouble(),
      normalizedQuery: map['normalized_query'] as String,
      queryFingerprint: map['query_fingerprint'] as String,
      rows: (map['rows'] as num).toInt(),
      totalExecTimeMs: (map['total_exec_time_ms'] as num).toDouble(),
    );
  }

  Map<String, dynamic> toJson() => {
    'calls': calls,
    'mean_exec_time_ms': meanExecTimeMs,
    'normalized_query': normalizedQuery,
    'query_fingerprint': queryFingerprint,
    'rows': rows,
    'total_exec_time_ms': totalExecTimeMs,
  };
}
