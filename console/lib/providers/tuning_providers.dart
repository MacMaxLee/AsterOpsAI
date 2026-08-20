import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// The tuning plan history view (unit U17) — read-only, same
/// `PolledRepository` + `StreamProvider.autoDispose` pattern as every
/// telemetry provider; no mutation, unlike `policy_providers.dart`'s own
/// grant/reject shape (U14's own endpoint is read-only too).
final tuningPlansProvider =
    StreamProvider.autoDispose<ApiResult<List<TuningPlanSummary>>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<List<TuningPlanSummary>>(
        fetch: client.getTuningPlans,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });
