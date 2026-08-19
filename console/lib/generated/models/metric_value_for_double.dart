// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class MetricValueForDouble {
  const MetricValueForDouble();

  static MetricValueForDouble fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return MetricValueForDoubleSupported.fromJson(map);
      case 'SAMPLE_GAP':
        return MetricValueForDoubleSampleGap.fromJson(map);
      case 'COUNTER_RESET':
        return MetricValueForDoubleCounterReset.fromJson(map);
      case 'UNAVAILABLE':
        return MetricValueForDoubleUnavailable.fromJson(map);
      default:
        throw FormatException('Unknown MetricValueForDouble tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class MetricValueForDoubleSupported extends MetricValueForDouble {
  final double value;
  const MetricValueForDoubleSupported({required this.value});

  static MetricValueForDoubleSupported fromJson(Map<String, dynamic> map) {
    return MetricValueForDoubleSupported(
      value: (map['value'] as num).toDouble(),
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SUPPORTED', 'value': value};
}

final class MetricValueForDoubleSampleGap extends MetricValueForDouble {
  final String reason;
  const MetricValueForDoubleSampleGap({required this.reason});

  static MetricValueForDoubleSampleGap fromJson(Map<String, dynamic> map) {
    return MetricValueForDoubleSampleGap(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SAMPLE_GAP', 'reason': reason};
}

final class MetricValueForDoubleCounterReset extends MetricValueForDouble {
  final String reason;
  const MetricValueForDoubleCounterReset({required this.reason});

  static MetricValueForDoubleCounterReset fromJson(Map<String, dynamic> map) {
    return MetricValueForDoubleCounterReset(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'COUNTER_RESET', 'reason': reason};
}

final class MetricValueForDoubleUnavailable extends MetricValueForDouble {
  final String reason;
  const MetricValueForDoubleUnavailable({required this.reason});

  static MetricValueForDoubleUnavailable fromJson(Map<String, dynamic> map) {
    return MetricValueForDoubleUnavailable(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}
