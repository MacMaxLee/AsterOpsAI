import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// The policy approval inbox (unit U16) — same `PolledRepository` +
/// `StreamProvider.autoDispose` pattern as every telemetry provider
/// (telemetry_providers.dart); grant/reject themselves are one-shot
/// `ApiClient` calls a screen makes directly, not something this provider
/// exposes (see `screens/policy_inbox_screen.dart`).
final pendingActionsProvider =
    StreamProvider.autoDispose<ApiResult<List<PendingActionSummary>>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<List<PendingActionSummary>>(
        fetch: client.getPendingActions,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });
