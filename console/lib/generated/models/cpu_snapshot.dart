// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'cpu_pressure.dart';
import 'metric_value_for_double.dart';

final class CpuSnapshot {
  final MetricValueForDouble aggregateUtilizationPercent;
  final bool containerized;
  final MetricValueForDouble contextSwitchesPerSec;
  final List<MetricValueForDouble> frequencyMhz;
  final MetricValueForDouble interruptsPerSec;
  final MetricValueForDouble loadAverage15m;
  final MetricValueForDouble loadAverage1m;
  final MetricValueForDouble loadAverage5m;
  final int logicalCoreCount;
  final List<MetricValueForDouble> perCoreUtilizationPercent;
  final CpuPressure pressure;
  final DateTime timestamp;

  const CpuSnapshot({
    required this.aggregateUtilizationPercent,
    required this.containerized,
    required this.contextSwitchesPerSec,
    required this.frequencyMhz,
    required this.interruptsPerSec,
    required this.loadAverage15m,
    required this.loadAverage1m,
    required this.loadAverage5m,
    required this.logicalCoreCount,
    required this.perCoreUtilizationPercent,
    required this.pressure,
    required this.timestamp,
  });

  static CpuSnapshot fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return CpuSnapshot(
      aggregateUtilizationPercent: MetricValueForDouble.fromJson(
        map['aggregate_utilization_percent'],
      ),
      containerized: map['containerized'] as bool,
      contextSwitchesPerSec: MetricValueForDouble.fromJson(
        map['context_switches_per_sec'],
      ),
      frequencyMhz: (map['frequency_mhz'] as List<dynamic>)
          .map((e) => MetricValueForDouble.fromJson(e))
          .toList(),
      interruptsPerSec: MetricValueForDouble.fromJson(
        map['interrupts_per_sec'],
      ),
      loadAverage15m: MetricValueForDouble.fromJson(map['load_average_15m']),
      loadAverage1m: MetricValueForDouble.fromJson(map['load_average_1m']),
      loadAverage5m: MetricValueForDouble.fromJson(map['load_average_5m']),
      logicalCoreCount: (map['logical_core_count'] as num).toInt(),
      perCoreUtilizationPercent:
          (map['per_core_utilization_percent'] as List<dynamic>)
              .map((e) => MetricValueForDouble.fromJson(e))
              .toList(),
      pressure: CpuPressure.fromJson(map['pressure']),
      timestamp: DateTime.parse(map['timestamp'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'aggregate_utilization_percent': aggregateUtilizationPercent.toJson(),
    'containerized': containerized,
    'context_switches_per_sec': contextSwitchesPerSec.toJson(),
    'frequency_mhz': frequencyMhz.map((e) => e.toJson()).toList(),
    'interrupts_per_sec': interruptsPerSec.toJson(),
    'load_average_15m': loadAverage15m.toJson(),
    'load_average_1m': loadAverage1m.toJson(),
    'load_average_5m': loadAverage5m.toJson(),
    'logical_core_count': logicalCoreCount,
    'per_core_utilization_percent': perCoreUtilizationPercent
        .map((e) => e.toJson())
        .toList(),
    'pressure': pressure.toJson(),
    'timestamp': timestamp.toIso8601String(),
  };
}
