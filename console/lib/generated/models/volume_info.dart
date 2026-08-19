// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'metric_value_for_double.dart';
import 'metric_value_for_uint64.dart';

final class VolumeInfo {
  final MetricValueForUint64 availableBytes;
  final MetricValueForUint64 capacityBytes;
  final String device;
  final String filesystem;
  final MetricValueForUint64 freeBytes;
  final MetricValueForDouble ioLatencyMs;
  final String mountPoint;
  final MetricValueForDouble readBytesPerSec;
  final MetricValueForDouble readOpsPerSec;
  final MetricValueForDouble writeBytesPerSec;
  final MetricValueForDouble writeOpsPerSec;

  const VolumeInfo({
    required this.availableBytes,
    required this.capacityBytes,
    required this.device,
    required this.filesystem,
    required this.freeBytes,
    required this.ioLatencyMs,
    required this.mountPoint,
    required this.readBytesPerSec,
    required this.readOpsPerSec,
    required this.writeBytesPerSec,
    required this.writeOpsPerSec,
  });

  static VolumeInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return VolumeInfo(
      availableBytes: MetricValueForUint64.fromJson(map['available_bytes']),
      capacityBytes: MetricValueForUint64.fromJson(map['capacity_bytes']),
      device: map['device'] as String,
      filesystem: map['filesystem'] as String,
      freeBytes: MetricValueForUint64.fromJson(map['free_bytes']),
      ioLatencyMs: MetricValueForDouble.fromJson(map['io_latency_ms']),
      mountPoint: map['mount_point'] as String,
      readBytesPerSec: MetricValueForDouble.fromJson(map['read_bytes_per_sec']),
      readOpsPerSec: MetricValueForDouble.fromJson(map['read_ops_per_sec']),
      writeBytesPerSec: MetricValueForDouble.fromJson(
        map['write_bytes_per_sec'],
      ),
      writeOpsPerSec: MetricValueForDouble.fromJson(map['write_ops_per_sec']),
    );
  }

  Map<String, dynamic> toJson() => {
    'available_bytes': availableBytes.toJson(),
    'capacity_bytes': capacityBytes.toJson(),
    'device': device,
    'filesystem': filesystem,
    'free_bytes': freeBytes.toJson(),
    'io_latency_ms': ioLatencyMs.toJson(),
    'mount_point': mountPoint,
    'read_bytes_per_sec': readBytesPerSec.toJson(),
    'read_ops_per_sec': readOpsPerSec.toJson(),
    'write_bytes_per_sec': writeBytesPerSec.toJson(),
    'write_ops_per_sec': writeOpsPerSec.toJson(),
  };
}
