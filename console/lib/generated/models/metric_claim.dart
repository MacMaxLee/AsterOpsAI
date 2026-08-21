// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class MetricClaim {
  final int evidenceRef;
  final double value;

  const MetricClaim({required this.evidenceRef, required this.value});

  static MetricClaim fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return MetricClaim(
      evidenceRef: (map['evidence_ref'] as num).toInt(),
      value: (map['value'] as num).toDouble(),
    );
  }

  Map<String, dynamic> toJson() => {
    'evidence_ref': evidenceRef,
    'value': value,
  };
}
