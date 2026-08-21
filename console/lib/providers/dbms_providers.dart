import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// Unit U32's console surface for unit U31's own real endpoints — two
/// independent providers, same `PolledRepository` + `StreamProvider.
/// autoDispose` pattern every other polled provider already uses, so a
/// slow/failed poll on one (e.g. `dbmsLocksProvider`) never blocks the
/// other (`database_screen.dart` watches both independently).
final dbmsSessionsProvider =
    StreamProvider.autoDispose<ApiResult<List<SessionInfo>>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<List<SessionInfo>>(
        fetch: client.getDbmsSessions,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });

final dbmsLocksProvider = StreamProvider.autoDispose<ApiResult<List<LockEdge>>>(
  (ref) {
    final client = ref.watch(apiClientProvider);
    final repo = PolledRepository<List<LockEdge>>(
      fetch: client.getDbmsLocks,
      interval: ref.watch(refreshIntervalProvider),
    );
    ref.onDispose(repo.dispose);
    return repo.stream;
  },
);

/// Unit U34's own console surface for unit U33's own real endpoint.
final dbmsQueryStatsProvider =
    StreamProvider.autoDispose<ApiResult<GatedValueForArrayOfQueryStat>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<GatedValueForArrayOfQueryStat>(
        fetch: client.getDbmsQueryStats,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });

/// Unit U36's own console surface for unit U35's own real endpoints.
final dbmsTableStatsProvider =
    StreamProvider.autoDispose<ApiResult<List<TableStat>>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<List<TableStat>>(
        fetch: client.getDbmsTableStats,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });

final dbmsIndexStatsProvider =
    StreamProvider.autoDispose<ApiResult<List<IndexStat>>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<List<IndexStat>>(
        fetch: client.getDbmsIndexStats,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });
