import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_result.dart';
import '../generated/models/models.dart';
import '../repositories/polled_repository.dart';
import 'settings_provider.dart';
import 'transport_provider.dart';

/// The security triage view (unit U18) — read-only, same
/// `PolledRepository` + `StreamProvider.autoDispose` pattern as
/// `tuning_providers.dart`. Suppression is a standalone action, not
/// something this provider exposes (see `screens/
/// security_incidents_screen.dart` — the wire type carries no
/// `detector_id`/resource to tie a suppression to a specific incident
/// row).
final openIncidentsProvider =
    StreamProvider.autoDispose<ApiResult<List<SecurityIncidentSummary>>>((ref) {
      final client = ref.watch(apiClientProvider);
      final repo = PolledRepository<List<SecurityIncidentSummary>>(
        fetch: client.getOpenIncidents,
        interval: ref.watch(refreshIntervalProvider),
      );
      ref.onDispose(repo.dispose);
      return repo.stream;
    });
