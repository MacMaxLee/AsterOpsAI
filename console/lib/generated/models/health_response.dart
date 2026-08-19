// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'capability.dart';
import 'self_metric_value_for_double.dart';
import 'self_metric_value_for_uint64.dart';

final class HealthResponse {
  final String apiVersion;
  final String arch;
  final Map<String, Capability> capabilities;
  final String name;
  final String platform;
  final SelfMetricValueForDouble selfCpuPercent;
  final SelfMetricValueForUint64 selfRssBytes;
  final int uptimeSeconds;
  final String version;

  const HealthResponse({
    required this.apiVersion,
    required this.arch,
    required this.capabilities,
    required this.name,
    required this.platform,
    required this.selfCpuPercent,
    required this.selfRssBytes,
    required this.uptimeSeconds,
    required this.version,
  });

  static HealthResponse fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return HealthResponse(
      apiVersion: map['api_version'] as String,
      arch: map['arch'] as String,
      capabilities: (map['capabilities'] as Map<String, dynamic>).map(
        (k, v) => MapEntry(k, Capability.fromJson(v)),
      ),
      name: map['name'] as String,
      platform: map['platform'] as String,
      selfCpuPercent: SelfMetricValueForDouble.fromJson(
        map['self_cpu_percent'],
      ),
      selfRssBytes: SelfMetricValueForUint64.fromJson(map['self_rss_bytes']),
      uptimeSeconds: (map['uptime_seconds'] as num).toInt(),
      version: map['version'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'api_version': apiVersion,
    'arch': arch,
    'capabilities': capabilities.map((k, v) => MapEntry(k, v.toJson())),
    'name': name,
    'platform': platform,
    'self_cpu_percent': selfCpuPercent.toJson(),
    'self_rss_bytes': selfRssBytes.toJson(),
    'uptime_seconds': uptimeSeconds,
    'version': version,
  };
}
