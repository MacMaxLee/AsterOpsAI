// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'memory_pressure.dart';
import 'metric_value_for_array_of_numa_node_memory.dart';
import 'metric_value_for_uint64.dart';

final class MemorySnapshot {
  final MetricValueForUint64 availableBytes;
  final MetricValueForUint64 buffersBytes;
  final MetricValueForUint64 cachedBytes;
  final bool containerized;
  final MetricValueForArrayOfNumaNodeMemory numaNodes;
  final MemoryPressure pressure;
  final MetricValueForUint64 swapFreeBytes;
  final MetricValueForUint64 swapTotalBytes;
  final MetricValueForUint64 swapUsedBytes;
  final DateTime timestamp;
  final MetricValueForUint64 totalBytes;
  final MetricValueForUint64 usedBytes;

  const MemorySnapshot({
    required this.availableBytes,
    required this.buffersBytes,
    required this.cachedBytes,
    required this.containerized,
    required this.numaNodes,
    required this.pressure,
    required this.swapFreeBytes,
    required this.swapTotalBytes,
    required this.swapUsedBytes,
    required this.timestamp,
    required this.totalBytes,
    required this.usedBytes,
  });

  static MemorySnapshot fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return MemorySnapshot(
      availableBytes: MetricValueForUint64.fromJson(map['available_bytes']),
      buffersBytes: MetricValueForUint64.fromJson(map['buffers_bytes']),
      cachedBytes: MetricValueForUint64.fromJson(map['cached_bytes']),
      containerized: map['containerized'] as bool,
      numaNodes: MetricValueForArrayOfNumaNodeMemory.fromJson(
        map['numa_nodes'],
      ),
      pressure: MemoryPressure.fromJson(map['pressure']),
      swapFreeBytes: MetricValueForUint64.fromJson(map['swap_free_bytes']),
      swapTotalBytes: MetricValueForUint64.fromJson(map['swap_total_bytes']),
      swapUsedBytes: MetricValueForUint64.fromJson(map['swap_used_bytes']),
      timestamp: DateTime.parse(map['timestamp'] as String),
      totalBytes: MetricValueForUint64.fromJson(map['total_bytes']),
      usedBytes: MetricValueForUint64.fromJson(map['used_bytes']),
    );
  }

  Map<String, dynamic> toJson() => {
    'available_bytes': availableBytes.toJson(),
    'buffers_bytes': buffersBytes.toJson(),
    'cached_bytes': cachedBytes.toJson(),
    'containerized': containerized,
    'numa_nodes': numaNodes.toJson(),
    'pressure': pressure.toJson(),
    'swap_free_bytes': swapFreeBytes.toJson(),
    'swap_total_bytes': swapTotalBytes.toJson(),
    'swap_used_bytes': swapUsedBytes.toJson(),
    'timestamp': timestamp.toIso8601String(),
    'total_bytes': totalBytes.toJson(),
    'used_bytes': usedBytes.toJson(),
  };
}
