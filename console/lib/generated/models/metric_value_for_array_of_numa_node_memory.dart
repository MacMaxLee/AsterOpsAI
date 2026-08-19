// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'numa_node_memory.dart';

sealed class MetricValueForArrayOfNumaNodeMemory {
  const MetricValueForArrayOfNumaNodeMemory();

  static MetricValueForArrayOfNumaNodeMemory fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    final tag = map['state'] as String;
    switch (tag) {
      case 'SUPPORTED':
        return MetricValueForArrayOfNumaNodeMemorySupported.fromJson(map);
      case 'SAMPLE_GAP':
        return MetricValueForArrayOfNumaNodeMemorySampleGap.fromJson(map);
      case 'COUNTER_RESET':
        return MetricValueForArrayOfNumaNodeMemoryCounterReset.fromJson(map);
      case 'UNAVAILABLE':
        return MetricValueForArrayOfNumaNodeMemoryUnavailable.fromJson(map);
      default:
        throw FormatException(
          'Unknown MetricValueForArrayOfNumaNodeMemory tag: $tag',
        );
    }
  }

  Map<String, dynamic> toJson();
}

final class MetricValueForArrayOfNumaNodeMemorySupported
    extends MetricValueForArrayOfNumaNodeMemory {
  final List<NumaNodeMemory> value;
  const MetricValueForArrayOfNumaNodeMemorySupported({required this.value});

  static MetricValueForArrayOfNumaNodeMemorySupported fromJson(
    Map<String, dynamic> map,
  ) {
    return MetricValueForArrayOfNumaNodeMemorySupported(
      value: (map['value'] as List<dynamic>)
          .map((e) => NumaNodeMemory.fromJson(e))
          .toList(),
    );
  }

  @override
  Map<String, dynamic> toJson() => {
    'state': 'SUPPORTED',
    'value': value.map((e) => e.toJson()).toList(),
  };
}

final class MetricValueForArrayOfNumaNodeMemorySampleGap
    extends MetricValueForArrayOfNumaNodeMemory {
  final String reason;
  const MetricValueForArrayOfNumaNodeMemorySampleGap({required this.reason});

  static MetricValueForArrayOfNumaNodeMemorySampleGap fromJson(
    Map<String, dynamic> map,
  ) {
    return MetricValueForArrayOfNumaNodeMemorySampleGap(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'SAMPLE_GAP', 'reason': reason};
}

final class MetricValueForArrayOfNumaNodeMemoryCounterReset
    extends MetricValueForArrayOfNumaNodeMemory {
  final String reason;
  const MetricValueForArrayOfNumaNodeMemoryCounterReset({required this.reason});

  static MetricValueForArrayOfNumaNodeMemoryCounterReset fromJson(
    Map<String, dynamic> map,
  ) {
    return MetricValueForArrayOfNumaNodeMemoryCounterReset(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'COUNTER_RESET', 'reason': reason};
}

final class MetricValueForArrayOfNumaNodeMemoryUnavailable
    extends MetricValueForArrayOfNumaNodeMemory {
  final String reason;
  const MetricValueForArrayOfNumaNodeMemoryUnavailable({required this.reason});

  static MetricValueForArrayOfNumaNodeMemoryUnavailable fromJson(
    Map<String, dynamic> map,
  ) {
    return MetricValueForArrayOfNumaNodeMemoryUnavailable(
      reason: map['reason'] as String,
    );
  }

  @override
  Map<String, dynamic> toJson() => {'state': 'UNAVAILABLE', 'reason': reason};
}
