// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'capability.dart';
import 'metric_value_for_double.dart';
import 'metric_value_for_string.dart';
import 'metric_value_for_uint64.dart';
import 'process_category.dart';

final class ProcessInfo {
  final ProcessCategory category;
  final MetricValueForString cmdline;
  final String comm;
  final MetricValueForDouble cpuPercent;
  final Capability diskIoCapability;
  final MetricValueForDouble diskReadBytesPerSec;
  final MetricValueForDouble diskWriteBytesPerSec;
  final Capability networkIoCapability;
  final MetricValueForDouble networkRxBytesPerSec;
  final MetricValueForDouble networkTxBytesPerSec;
  final int ownerUid;
  final int pid;
  final MetricValueForUint64 rssBytes;
  final int startTimeTicks;

  const ProcessInfo({
    required this.category,
    required this.cmdline,
    required this.comm,
    required this.cpuPercent,
    required this.diskIoCapability,
    required this.diskReadBytesPerSec,
    required this.diskWriteBytesPerSec,
    required this.networkIoCapability,
    required this.networkRxBytesPerSec,
    required this.networkTxBytesPerSec,
    required this.ownerUid,
    required this.pid,
    required this.rssBytes,
    required this.startTimeTicks,
  });

  static ProcessInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return ProcessInfo(
      category: ProcessCategory.fromJson(map['category']),
      cmdline: MetricValueForString.fromJson(map['cmdline']),
      comm: map['comm'] as String,
      cpuPercent: MetricValueForDouble.fromJson(map['cpu_percent']),
      diskIoCapability: Capability.fromJson(map['disk_io_capability']),
      diskReadBytesPerSec: MetricValueForDouble.fromJson(
        map['disk_read_bytes_per_sec'],
      ),
      diskWriteBytesPerSec: MetricValueForDouble.fromJson(
        map['disk_write_bytes_per_sec'],
      ),
      networkIoCapability: Capability.fromJson(map['network_io_capability']),
      networkRxBytesPerSec: MetricValueForDouble.fromJson(
        map['network_rx_bytes_per_sec'],
      ),
      networkTxBytesPerSec: MetricValueForDouble.fromJson(
        map['network_tx_bytes_per_sec'],
      ),
      ownerUid: (map['owner_uid'] as num).toInt(),
      pid: (map['pid'] as num).toInt(),
      rssBytes: MetricValueForUint64.fromJson(map['rss_bytes']),
      startTimeTicks: (map['start_time_ticks'] as num).toInt(),
    );
  }

  Map<String, dynamic> toJson() => {
    'category': category.toJson(),
    'cmdline': cmdline.toJson(),
    'comm': comm,
    'cpu_percent': cpuPercent.toJson(),
    'disk_io_capability': diskIoCapability.toJson(),
    'disk_read_bytes_per_sec': diskReadBytesPerSec.toJson(),
    'disk_write_bytes_per_sec': diskWriteBytesPerSec.toJson(),
    'network_io_capability': networkIoCapability.toJson(),
    'network_rx_bytes_per_sec': networkRxBytesPerSec.toJson(),
    'network_tx_bytes_per_sec': networkTxBytesPerSec.toJson(),
    'owner_uid': ownerUid,
    'pid': pid,
    'rss_bytes': rssBytes.toJson(),
    'start_time_ticks': startTimeTicks,
  };
}
