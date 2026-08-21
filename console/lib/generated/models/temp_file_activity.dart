// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class TempFileActivity {
  final DateTime? statsReset;
  final int tempBytes;
  final int tempFiles;

  const TempFileActivity({
    this.statsReset,
    required this.tempBytes,
    required this.tempFiles,
  });

  static TempFileActivity fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return TempFileActivity(
      statsReset: map['stats_reset'] == null
          ? null
          : (DateTime.parse(map['stats_reset'] as String)),
      tempBytes: (map['temp_bytes'] as num).toInt(),
      tempFiles: (map['temp_files'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'stats_reset': statsReset?.toIso8601String(),
    'temp_bytes': tempBytes,
    'temp_files': tempFiles,
  };
}
