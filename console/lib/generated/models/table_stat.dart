// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class TableStat {
  final int idxScan;
  final DateTime? lastAutovacuum;
  final DateTime? lastVacuum;
  final int nDeadTup;
  final int nLiveTup;
  final String schema;
  final int seqScan;
  final String table;
  final int totalSizeBytes;

  const TableStat({
    required this.idxScan,
    this.lastAutovacuum,
    this.lastVacuum,
    required this.nDeadTup,
    required this.nLiveTup,
    required this.schema,
    required this.seqScan,
    required this.table,
    required this.totalSizeBytes,
  });

  static TableStat fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return TableStat(
      idxScan: (map['idx_scan'] as num).toInt(),
      lastAutovacuum: map['last_autovacuum'] == null
          ? null
          : (DateTime.parse(map['last_autovacuum'] as String)),
      lastVacuum: map['last_vacuum'] == null
          ? null
          : (DateTime.parse(map['last_vacuum'] as String)),
      nDeadTup: (map['n_dead_tup'] as num).toInt(),
      nLiveTup: (map['n_live_tup'] as num).toInt(),
      schema: map['schema'] as String,
      seqScan: (map['seq_scan'] as num).toInt(),
      table: map['table'] as String,
      totalSizeBytes: (map['total_size_bytes'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'idx_scan': idxScan,
    'last_autovacuum': lastAutovacuum?.toIso8601String(),
    'last_vacuum': lastVacuum?.toIso8601String(),
    'n_dead_tup': nDeadTup,
    'n_live_tup': nLiveTup,
    'schema': schema,
    'seq_scan': seqScan,
    'table': table,
    'total_size_bytes': totalSizeBytes,
  };
}
