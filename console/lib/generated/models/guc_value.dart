// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class GucValue {
  final String name;
  final String setting;
  final String source;
  final String? unit;

  const GucValue({
    required this.name,
    required this.setting,
    required this.source,
    this.unit,
  });

  static GucValue fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return GucValue(
      name: map['name'] as String,
      setting: map['setting'] as String,
      source: map['source'] as String,
      unit: map['unit'] == null ? null : (map['unit'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'name': name,
    'setting': setting,
    'source': source,
    'unit': unit,
  };
}
