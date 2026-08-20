// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum RootCause {
  dbLocks._('DB_LOCKS'),
  dbConfiguration._('DB_CONFIGURATION'),
  connectionExhaustion._('CONNECTION_EXHAUSTION'),
  slowSql._('SLOW_SQL'),
  hostCpu._('HOST_CPU'),
  hostMemory._('HOST_MEMORY'),
  storageLatency._('STORAGE_LATENCY'),
  network._('NETWORK'),
  clientSideApplication._('CLIENT_SIDE_APPLICATION');

  final String wireValue;
  const RootCause._(this.wireValue);

  static RootCause fromJson(dynamic json) {
    final value = json as String;
    return RootCause.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown RootCause: $value'),
    );
  }

  String toJson() => wireValue;
}
