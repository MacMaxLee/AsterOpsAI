// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class MetricValueForUint64 {
  const MetricValueForUint64();

  static MetricValueForUint64 fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return MetricValueForUint64Supported.fromJson(map);
      case 'SAMPLE_GAP':
        return MetricValueForUint64SampleGap.fromJson(map);
      case 'COUNTER_RESET':
        return MetricValueForUint64CounterReset.fromJson(map);
      case 'UNAVAILABLE':
        return MetricValueForUint64Unavailable.fromJson(map);
      default:
        throw FormatException('Unknown MetricValueForUint64 tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class MetricValueForUint64Supported extends MetricValueForUint64 {
  final int value;
  const MetricValueForUint64Supported({required this.value});

  static MetricValueForUint64Supported fromJson(Map<String, dynamic> map) {
    return MetricValueForUint64Supported(value: (map['value'] as num).toInt());
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SUPPORTED', 'value': value};
}

final class MetricValueForUint64SampleGap extends MetricValueForUint64 {
  final String reason;
  const MetricValueForUint64SampleGap({required this.reason});

  static MetricValueForUint64SampleGap fromJson(Map<String, dynamic> map) {
    return MetricValueForUint64SampleGap(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SAMPLE_GAP', 'reason': reason};
}

final class MetricValueForUint64CounterReset extends MetricValueForUint64 {
  final String reason;
  const MetricValueForUint64CounterReset({required this.reason});

  static MetricValueForUint64CounterReset fromJson(Map<String, dynamic> map) {
    return MetricValueForUint64CounterReset(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'COUNTER_RESET', 'reason': reason};
}

final class MetricValueForUint64Unavailable extends MetricValueForUint64 {
  final String reason;
  const MetricValueForUint64Unavailable({required this.reason});

  static MetricValueForUint64Unavailable fromJson(Map<String, dynamic> map) {
    return MetricValueForUint64Unavailable(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}
