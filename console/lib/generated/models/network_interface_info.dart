// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'metric_value_for_double.dart';

final class NetworkInterfaceInfo {
  final String name;
  final MetricValueForDouble rxBytesPerSec;
  final MetricValueForDouble rxDropsPerSec;
  final MetricValueForDouble rxErrorsPerSec;
  final MetricValueForDouble rxPacketsPerSec;
  final MetricValueForDouble txBytesPerSec;
  final MetricValueForDouble txDropsPerSec;
  final MetricValueForDouble txErrorsPerSec;
  final MetricValueForDouble txPacketsPerSec;

  const NetworkInterfaceInfo({
    required this.name,
    required this.rxBytesPerSec,
    required this.rxDropsPerSec,
    required this.rxErrorsPerSec,
    required this.rxPacketsPerSec,
    required this.txBytesPerSec,
    required this.txDropsPerSec,
    required this.txErrorsPerSec,
    required this.txPacketsPerSec,
  });

  static NetworkInterfaceInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return NetworkInterfaceInfo(
      name: map['name'] as String,
      rxBytesPerSec: MetricValueForDouble.fromJson(map['rx_bytes_per_sec']),
      rxDropsPerSec: MetricValueForDouble.fromJson(map['rx_drops_per_sec']),
      rxErrorsPerSec: MetricValueForDouble.fromJson(map['rx_errors_per_sec']),
      rxPacketsPerSec: MetricValueForDouble.fromJson(map['rx_packets_per_sec']),
      txBytesPerSec: MetricValueForDouble.fromJson(map['tx_bytes_per_sec']),
      txDropsPerSec: MetricValueForDouble.fromJson(map['tx_drops_per_sec']),
      txErrorsPerSec: MetricValueForDouble.fromJson(map['tx_errors_per_sec']),
      txPacketsPerSec: MetricValueForDouble.fromJson(map['tx_packets_per_sec']),
    );
  }

  Map<String, dynamic> toJson() => {
    'name': name,
    'rx_bytes_per_sec': rxBytesPerSec.toJson(),
    'rx_drops_per_sec': rxDropsPerSec.toJson(),
    'rx_errors_per_sec': rxErrorsPerSec.toJson(),
    'rx_packets_per_sec': rxPacketsPerSec.toJson(),
    'tx_bytes_per_sec': txBytesPerSec.toJson(),
    'tx_drops_per_sec': txDropsPerSec.toJson(),
    'tx_errors_per_sec': txErrorsPerSec.toJson(),
    'tx_packets_per_sec': txPacketsPerSec.toJson(),
  };
}
