// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class SelfMetricValueForDouble {
  const SelfMetricValueForDouble();

  static SelfMetricValueForDouble fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return SelfMetricValueForDoubleSupported.fromJson(map);
      case 'UNAVAILABLE':
        return SelfMetricValueForDoubleUnavailable.fromJson(map);
      default:
        throw FormatException('Unknown SelfMetricValueForDouble tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class SelfMetricValueForDoubleSupported extends SelfMetricValueForDouble {
  final double value;
  const SelfMetricValueForDoubleSupported({required this.value});

  static SelfMetricValueForDoubleSupported fromJson(Map<String, dynamic> map) {
    return SelfMetricValueForDoubleSupported(
      value: (map['value'] as num).toDouble(),
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SUPPORTED', 'value': value};
}

final class SelfMetricValueForDoubleUnavailable
    extends SelfMetricValueForDouble {
  final String reason;
  const SelfMetricValueForDoubleUnavailable({required this.reason});

  static SelfMetricValueForDoubleUnavailable fromJson(
    Map<String, dynamic> map,
  ) {
    return SelfMetricValueForDoubleUnavailable(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}
