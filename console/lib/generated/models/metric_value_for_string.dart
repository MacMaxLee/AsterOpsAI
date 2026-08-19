// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class MetricValueForString {
  const MetricValueForString();

  static MetricValueForString fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return MetricValueForStringSupported.fromJson(map);
      case 'SAMPLE_GAP':
        return MetricValueForStringSampleGap.fromJson(map);
      case 'COUNTER_RESET':
        return MetricValueForStringCounterReset.fromJson(map);
      case 'UNAVAILABLE':
        return MetricValueForStringUnavailable.fromJson(map);
      default:
        throw FormatException('Unknown MetricValueForString tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class MetricValueForStringSupported extends MetricValueForString {
  final String value;
  const MetricValueForStringSupported({required this.value});

  static MetricValueForStringSupported fromJson(Map<String, dynamic> map) {
    return MetricValueForStringSupported(value: map['value'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SUPPORTED', 'value': value};
}

final class MetricValueForStringSampleGap extends MetricValueForString {
  final String reason;
  const MetricValueForStringSampleGap({required this.reason});

  static MetricValueForStringSampleGap fromJson(Map<String, dynamic> map) {
    return MetricValueForStringSampleGap(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SAMPLE_GAP', 'reason': reason};
}

final class MetricValueForStringCounterReset extends MetricValueForString {
  final String reason;
  const MetricValueForStringCounterReset({required this.reason});

  static MetricValueForStringCounterReset fromJson(Map<String, dynamic> map) {
    return MetricValueForStringCounterReset(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'COUNTER_RESET', 'reason': reason};
}

final class MetricValueForStringUnavailable extends MetricValueForString {
  final String reason;
  const MetricValueForStringUnavailable({required this.reason});

  static MetricValueForStringUnavailable fromJson(Map<String, dynamic> map) {
    return MetricValueForStringUnavailable(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}
