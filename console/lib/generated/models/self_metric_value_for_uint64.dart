// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

sealed class SelfMetricValueForUint64 {
  const SelfMetricValueForUint64();

  static SelfMetricValueForUint64 fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return SelfMetricValueForUint64Supported.fromJson(map);
      case 'UNAVAILABLE':
        return SelfMetricValueForUint64Unavailable.fromJson(map);
      default:
        throw FormatException('Unknown SelfMetricValueForUint64 tag: $tag');
    }
  }

  Map<String, dynamic> toJson();
}

final class SelfMetricValueForUint64Supported extends SelfMetricValueForUint64 {
  final int value;
  const SelfMetricValueForUint64Supported({required this.value});

  static SelfMetricValueForUint64Supported fromJson(Map<String, dynamic> map) {
    return SelfMetricValueForUint64Supported(
      value: (map['value'] as num).toInt(),
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SUPPORTED', 'value': value};
}

final class SelfMetricValueForUint64Unavailable
    extends SelfMetricValueForUint64 {
  final String reason;
  const SelfMetricValueForUint64Unavailable({required this.reason});

  static SelfMetricValueForUint64Unavailable fromJson(
    Map<String, dynamic> map,
  ) {
    return SelfMetricValueForUint64Unavailable(reason: map['reason'] as String);
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}
