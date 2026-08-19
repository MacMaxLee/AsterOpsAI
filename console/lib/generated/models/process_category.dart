// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum ProcessCategory {
  system._('SYSTEM'),
  userApplication._('USER_APPLICATION'),
  backgroundService._('BACKGROUND_SERVICE'),
  dbmsEngine._('DBMS_ENGINE'),
  unknown._('UNKNOWN');

  final String wireValue;
  const ProcessCategory._(this.wireValue);

  static ProcessCategory fromJson(dynamic json) {
    final value = json as String;
    return ProcessCategory.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown ProcessCategory: $value'),
    );
  }

  String toJson() => wireValue;
}
