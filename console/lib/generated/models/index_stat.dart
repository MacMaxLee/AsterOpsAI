// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class IndexStat {
  final int idxScan;
  final String index;
  final String schema;
  final int sizeBytes;
  final String table;

  const IndexStat({
    required this.idxScan,
    required this.index,
    required this.schema,
    required this.sizeBytes,
    required this.table,
  });

  static IndexStat fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return IndexStat(
      idxScan: (map['idx_scan'] as num).toInt(),
      index: map['index'] as String,
      schema: map['schema'] as String,
      sizeBytes: (map['size_bytes'] as num).toInt(),
      table: map['table'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'idx_scan': idxScan,
    'index': index,
    'schema': schema,
    'size_bytes': sizeBytes,
    'table': table,
  };
}
