import 'package:console/api/api_version.dart';
import 'package:console/generated/models/models.dart';

Map<String, dynamic> okEnvelopeJson(dynamic dataJson) => {
  'success': true,
  'timestamp': DateTime.now().toUtc().toIso8601String(),
  'request_id': 'test-request-id',
  'data': dataJson,
  'error': null,
};

Map<String, dynamic> errEnvelopeJson(ApiError error) => {
  'success': false,
  'timestamp': DateTime.now().toUtc().toIso8601String(),
  'request_id': 'test-request-id',
  'data': null,
  'error': error.toJson(),
};

HealthResponse fakeHealth({String apiVersion = kSupportedApiVersion}) =>
    HealthResponse(
      apiVersion: apiVersion,
      arch: 'aarch64',
      capabilities: const {},
      name: 'ai-ops-core',
      platform: 'linux',
      selfCpuPercent: const SelfMetricValueForDoubleSupported(value: 0.5),
      selfRssBytes: const SelfMetricValueForUint64Supported(value: 9000000),
      uptimeSeconds: 120,
      version: '0.4.0',
    );

CpuSnapshot fakeCpuSnapshot({
  MetricValueForDouble? aggregateUtilizationPercent,
}) => CpuSnapshot(
  aggregateUtilizationPercent:
      aggregateUtilizationPercent ??
      const MetricValueForDoubleSupported(value: 12.5),
  containerized: false,
  contextSwitchesPerSec: const MetricValueForDoubleSupported(value: 500),
  frequencyMhz: const [MetricValueForDoubleSupported(value: 3200)],
  interruptsPerSec: const MetricValueForDoubleSupported(value: 100),
  loadAverage15m: const MetricValueForDoubleSupported(value: 0.4),
  loadAverage1m: const MetricValueForDoubleSupported(value: 0.5),
  loadAverage5m: const MetricValueForDoubleSupported(value: 0.45),
  logicalCoreCount: 8,
  perCoreUtilizationPercent: const [
    MetricValueForDoubleSupported(value: 10),
    MetricValueForDoubleSupported(value: 15),
  ],
  pressure: CpuPressure.normal,
  timestamp: DateTime.now().toUtc(),
);

MemorySnapshot fakeMemorySnapshot() => MemorySnapshot(
  availableBytes: const MetricValueForUint64Supported(value: 4000000000),
  buffersBytes: const MetricValueForUint64Supported(value: 100000000),
  cachedBytes: const MetricValueForUint64Supported(value: 200000000),
  containerized: false,
  numaNodes: const MetricValueForArrayOfNumaNodeMemoryUnavailable(
    reason: 'single NUMA node',
  ),
  pressure: MemoryPressure.normal,
  swapFreeBytes: const MetricValueForUint64Supported(value: 1000000000),
  swapTotalBytes: const MetricValueForUint64Supported(value: 1000000000),
  swapUsedBytes: const MetricValueForUint64Supported(value: 0),
  timestamp: DateTime.now().toUtc(),
  totalBytes: const MetricValueForUint64Supported(value: 8000000000),
  usedBytes: const MetricValueForUint64Supported(value: 4000000000),
);

StorageSnapshot fakeStorageSnapshot() => StorageSnapshot(
  timestamp: DateTime.now().toUtc(),
  volumes: const [
    VolumeInfo(
      availableBytes: MetricValueForUint64Supported(value: 100000000000),
      capacityBytes: MetricValueForUint64Supported(value: 500000000000),
      device: '/dev/sda1',
      filesystem: 'ext4',
      freeBytes: MetricValueForUint64Supported(value: 100000000000),
      ioLatencyMs: MetricValueForDoubleSupported(value: 1.2),
      mountPoint: '/',
      readBytesPerSec: MetricValueForDoubleSupported(value: 1000),
      readOpsPerSec: MetricValueForDoubleSupported(value: 10),
      writeBytesPerSec: MetricValueForDoubleSupported(value: 2000),
      writeOpsPerSec: MetricValueForDoubleSupported(value: 20),
    ),
  ],
);

NetworkSnapshot fakeNetworkSnapshot() => NetworkSnapshot(
  interfaces: const [
    NetworkInterfaceInfo(
      name: 'eth0',
      rxBytesPerSec: MetricValueForDoubleSupported(value: 1000),
      rxDropsPerSec: MetricValueForDoubleSupported(value: 0),
      rxErrorsPerSec: MetricValueForDoubleSupported(value: 0),
      rxPacketsPerSec: MetricValueForDoubleSupported(value: 10),
      txBytesPerSec: MetricValueForDoubleSupported(value: 2000),
      txDropsPerSec: MetricValueForDoubleSupported(value: 0),
      txErrorsPerSec: MetricValueForDoubleSupported(value: 0),
      txPacketsPerSec: MetricValueForDoubleSupported(value: 20),
    ),
  ],
  timestamp: DateTime.now().toUtc(),
);

ProcessSnapshot fakeProcessSnapshot() => ProcessSnapshot(
  processes: const [
    ProcessInfo(
      category: ProcessCategory.userApplication,
      cmdline: MetricValueForStringSupported(value: '/usr/bin/example'),
      comm: 'example',
      cpuPercent: MetricValueForDoubleSupported(value: 1.5),
      diskIoCapability: CapabilitySupported(),
      diskReadBytesPerSec: MetricValueForDoubleSupported(value: 0),
      diskWriteBytesPerSec: MetricValueForDoubleSupported(value: 0),
      networkIoCapability: CapabilityPermissionRequired(
        reason: 'requires elevated privileges',
      ),
      networkRxBytesPerSec: MetricValueForDoubleUnavailable(
        reason: 'permission required',
      ),
      networkTxBytesPerSec: MetricValueForDoubleUnavailable(
        reason: 'permission required',
      ),
      ownerUid: 1000,
      pid: 4242,
      rssBytes: MetricValueForUint64Supported(value: 15000000),
      startTimeTicks: 12345,
    ),
  ],
  timestamp: DateTime.now().toUtc(),
  totalCount: 1,
);

DeviceSnapshot fakeDeviceSnapshot() => DeviceSnapshot(
  devices: const [
    DeviceInfo(
      identifier: 'sda',
      kind: DeviceKind.blockStorage,
      name: 'Primary disk',
      removable: false,
      trusted: true,
    ),
  ],
  timestamp: DateTime.now().toUtc(),
);

SystemStatusResponse fakeSystemStatusResponse() => SystemStatusResponse(
  capabilities: const {},
  containerized: false,
  cpuPressure: CpuPressure.normal,
  memoryPressure: MemoryPressure.normal,
  sampleIntervalMs: 1000,
  timestamp: DateTime.now().toUtc(),
  uptimeSeconds: 120,
);
