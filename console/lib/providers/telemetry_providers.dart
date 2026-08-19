import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// One `StreamProvider` per data family, each backed by its own
/// `PolledRepository` at the user's configured refresh interval
/// (Settings). `autoDispose` so a screen that isn't visible stops polling
/// instead of accumulating background timers (Riverpod tears down the
/// repository, which cancels its Timer, once nothing is watching).

final cpuProvider = StreamProvider.autoDispose<ApiResult<CpuSnapshot>>((ref) {
  final client = ref.watch(apiClientProvider);
  final repo = PolledRepository<CpuSnapshot>(
    fetch: client.getCpu,
    interval: ref.watch(refreshIntervalProvider),
  );
  ref.onDispose(repo.dispose);
  return repo.stream;
});

final memoryProvider = StreamProvider.autoDispose<ApiResult<MemorySnapshot>>((
  ref,
) {
  final client = ref.watch(apiClientProvider);
  final repo = PolledRepository<MemorySnapshot>(
    fetch: client.getMemory,
    interval: ref.watch(refreshIntervalProvider),
  );
  ref.onDispose(repo.dispose);
  return repo.stream;
});

final storageProvider = StreamProvider.autoDispose<ApiResult<StorageSnapshot>>((
  ref,
) {
  final client = ref.watch(apiClientProvider);
  final repo = PolledRepository<StorageSnapshot>(
    fetch: client.getStorage,
    interval: ref.watch(refreshIntervalProvider),
  );
  ref.onDispose(repo.dispose);
  return repo.stream;
});

final networkProvider = StreamProvider.autoDispose<ApiResult<NetworkSnapshot>>((
  ref,
) {
  final client = ref.watch(apiClientProvider);
  final repo = PolledRepository<NetworkSnapshot>(
    fetch: client.getNetwork,
    interval: ref.watch(refreshIntervalProvider),
  );
  ref.onDispose(repo.dispose);
  return repo.stream;
});

final processesProvider =
    StreamProvider.autoDispose<ApiResult<ProcessSnapshot>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<ProcessSnapshot>(
        fetch: client.getProcesses,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });

final devicesProvider = StreamProvider.autoDispose<ApiResult<DeviceSnapshot>>((
  ref,
) {
  final client = ref.watch(apiClientProvider);
  final repo = PolledRepository<DeviceSnapshot>(
    fetch: client.getDevices,
    interval: ref.watch(refreshIntervalProvider),
  );
  ref.onDispose(repo.dispose);
  return repo.stream;
});

final systemStatusProvider =
    StreamProvider.autoDispose<ApiResult<SystemStatusResponse>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<SystemStatusResponse>(
        fetch: client.getSystemStatus,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });
