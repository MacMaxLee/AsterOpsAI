import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// The cross-layer correlation verdict view (unit U20) — read-only, same
/// `PolledRepository` + `StreamProvider.autoDispose` pattern as
/// `analysis_providers.dart`.
final correlationProvider =
    StreamProvider.autoDispose<ApiResult<CorrelationResult>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<CorrelationResult>(
        fetch: client.getCorrelation,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });
