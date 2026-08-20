import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// The host performance analysis view (unit U19) — read-only, same
/// `PolledRepository` + `StreamProvider.autoDispose` pattern as
/// `tuning_providers.dart`.
final hostAnalysisProvider = StreamProvider.autoDispose<ApiResult<HostVerdict>>(
  (ref) {
    final client = ref.watch(apiClientProvider);
    final repo = PolledRepository<HostVerdict>(
      fetch: client.getHostAnalysis,
      interval: ref.watch(refreshIntervalProvider),
    );
    ref.onDispose(repo.dispose);
    return repo.stream;
  },
);
