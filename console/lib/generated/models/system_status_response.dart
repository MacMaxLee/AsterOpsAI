// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'capability.dart';
import 'cpu_pressure.dart';
import 'memory_pressure.dart';

final class SystemStatusResponse {
  final Map<String, Capability> capabilities;
  final bool containerized;
  final CpuPressure cpuPressure;
  final MemoryPressure memoryPressure;
  final int sampleIntervalMs;
  final DateTime timestamp;
  final int uptimeSeconds;

  const SystemStatusResponse({
    required this.capabilities,
    required this.containerized,
    required this.cpuPressure,
    required this.memoryPressure,
    required this.sampleIntervalMs,
    required this.timestamp,
    required this.uptimeSeconds,
  });

  static SystemStatusResponse fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return SystemStatusResponse(
      capabilities: (map['capabilities'] as Map<String, dynamic>).map(
        (k, v) => MapEntry(k, Capability.fromJson(v)),
      ),
      containerized: map['containerized'] as bool,
      cpuPressure: CpuPressure.fromJson(map['cpu_pressure']),
      memoryPressure: MemoryPressure.fromJson(map['memory_pressure']),
      sampleIntervalMs: (map['sample_interval_ms'] as num).toInt(),
      timestamp: DateTime.parse(map['timestamp'] as String),
      uptimeSeconds: (map['uptime_seconds'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'capabilities': capabilities.map((k, v) => MapEntry(k, v.toJson())),
    'containerized': containerized,
    'cpu_pressure': cpuPressure.toJson(),
    'memory_pressure': memoryPressure.toJson(),
    'sample_interval_ms': sampleIntervalMs,
    'timestamp': timestamp.toIso8601String(),
    'uptime_seconds': uptimeSeconds,
  };
}
