import 'dart:convert';

import '../generated/models/models.dart';
import 'api_failure.dart';
import 'api_result.dart';
import 'envelope.dart';
import 'history_query.dart';
import 'local_transport.dart';

/// The one seam through which every screen ultimately reaches the core.
/// Widgets never touch this directly (TRS §15) — only repositories do.
final class ApiClient {
  final LocalTransport _transport;

  ApiClient(this._transport);

  Future<ApiResult<HealthResponse>> getHealth() =>
      _get('/api/v1/health', HealthResponse.fromJson);

  Future<ApiResult<CpuSnapshot>> getCpu() =>
      _get('/api/v1/cpu', CpuSnapshot.fromJson);

  Future<ApiResult<MemorySnapshot>> getMemory() =>
      _get('/api/v1/memory', MemorySnapshot.fromJson);

  Future<ApiResult<StorageSnapshot>> getStorage() =>
      _get('/api/v1/storage', StorageSnapshot.fromJson);

  Future<ApiResult<NetworkSnapshot>> getNetwork() =>
      _get('/api/v1/network', NetworkSnapshot.fromJson);

  Future<ApiResult<ProcessSnapshot>> getProcesses() =>
      _get('/api/v1/processes', ProcessSnapshot.fromJson);

  Future<ApiResult<DeviceSnapshot>> getDevices() =>
      _get('/api/v1/devices', DeviceSnapshot.fromJson);

  Future<ApiResult<SystemStatusResponse>> getSystemStatus() =>
      _get('/api/v1/system/status', SystemStatusResponse.fromJson);

  Future<ApiResult<HistoryResponseForCpuHistoryPoint>> getCpuHistory(
    HistoryQuery query,
  ) => _get(
    '/api/v1/history/cpu?${query.toQueryString()}',
    HistoryResponseForCpuHistoryPoint.fromJson,
  );

  Future<ApiResult<HistoryResponseForMemoryHistoryPoint>> getMemoryHistory(
    HistoryQuery query,
  ) => _get(
    '/api/v1/history/memory?${query.toQueryString()}',
    HistoryResponseForMemoryHistoryPoint.fromJson,
  );

  Future<ApiResult<HistoryResponseForStorageHistoryPoint>> getStorageHistory(
    HistoryQuery query,
  ) => _get(
    '/api/v1/history/storage?${query.toQueryString()}',
    HistoryResponseForStorageHistoryPoint.fromJson,
  );

  Future<ApiResult<HistoryResponseForNetworkHistoryPoint>> getNetworkHistory(
    HistoryQuery query,
  ) => _get(
    '/api/v1/history/network?${query.toQueryString()}',
    HistoryResponseForNetworkHistoryPoint.fromJson,
  );

  Future<ApiResult<List<PendingActionSummary>>> getPendingActions() => _get(
    '/api/v1/policy/pending',
    (json) => (json as List<dynamic>)
        .map((e) => PendingActionSummary.fromJson(e))
        .toList(),
  );

  Future<ApiResult<void>> grantAction(int id, String grantedBy) =>
      _postForSuccess(
        '/api/v1/policy/$id/grant',
        jsonEncode({'granted_by': grantedBy}),
      );

  Future<ApiResult<void>> rejectAction(int id, String rejectedBy) =>
      _postForSuccess(
        '/api/v1/policy/$id/reject',
        jsonEncode({'rejected_by': rejectedBy}),
      );

  /// Unit U28's own rollback endpoint — returns `()`, same shape as
  /// grant/reject.
  Future<ApiResult<void>> rollbackAction(int id, String rolledBackBy) =>
      _postForSuccess(
        '/api/v1/policy/$id/rollback',
        jsonEncode({'rolled_back_by': rolledBackBy}),
      );

  /// Unit U29: the discovery step a real "Resume" affordance needs —
  /// 0 or 1 items, never sent as `null` (see `ActionProposalOutcome`'s
  /// own reasoning), so this is a plain list `_get`, not a special case.
  Future<ApiResult<List<ResumableActionSummary>>> getResumableActions({
    required int pid,
    required int startTimeTicks,
  }) => _get(
    '/api/v1/actions/resumable?pid=$pid&start_time_ticks=$startTimeTicks',
    (json) => (json as List<dynamic>)
        .map((e) => ResumableActionSummary.fromJson(e))
        .toList(),
  );

  Future<ApiResult<List<TuningPlanSummary>>> getTuningPlans() => _get(
    '/api/v1/tuning/plans',
    (json) => (json as List<dynamic>)
        .map((e) => TuningPlanSummary.fromJson(e))
        .toList(),
  );

  Future<ApiResult<List<SecurityIncidentSummary>>> getOpenIncidents() => _get(
    '/api/v1/security/incidents',
    (json) => (json as List<dynamic>)
        .map((e) => SecurityIncidentSummary.fromJson(e))
        .toList(),
  );

  Future<ApiResult<List<SessionInfo>>> getDbmsSessions() => _get(
    '/api/v1/dbms/sessions',
    (json) =>
        (json as List<dynamic>).map((e) => SessionInfo.fromJson(e)).toList(),
  );

  Future<ApiResult<List<LockEdge>>> getDbmsLocks() => _get(
    '/api/v1/dbms/locks',
    (json) => (json as List<dynamic>).map((e) => LockEdge.fromJson(e)).toList(),
  );

  /// Unlike `getDbmsSessions`/`getDbmsLocks`, the envelope's `data` is
  /// the gated value itself, not a bare list — `pg_stat_statements` may
  /// genuinely not be installed (unit U33).
  Future<ApiResult<GatedValueForArrayOfQueryStat>> getDbmsQueryStats() =>
      _get('/api/v1/dbms/query-stats', GatedValueForArrayOfQueryStat.fromJson);

  Future<ApiResult<List<TableStat>>> getDbmsTableStats() => _get(
    '/api/v1/dbms/table-stats',
    (json) => (json as List<dynamic>).map((e) => TableStat.fromJson(e)).toList(),
  );

  Future<ApiResult<List<IndexStat>>> getDbmsIndexStats() => _get(
    '/api/v1/dbms/index-stats',
    (json) => (json as List<dynamic>).map((e) => IndexStat.fromJson(e)).toList(),
  );

  /// Unlike every other `/dbms/*` endpoint, the envelope's `data` is a
  /// single object, not a list (unit U37).
  Future<ApiResult<ReplicationStatus>> getDbmsReplication() =>
      _get('/api/v1/dbms/replication', ReplicationStatus.fromJson);

  Future<ApiResult<List<GucValue>>> getDbmsGucs() => _get(
    '/api/v1/dbms/gucs',
    (json) => (json as List<dynamic>).map((e) => GucValue.fromJson(e)).toList(),
  );

  Future<ApiResult<TempFileActivity>> getDbmsTempFileActivity() =>
      _get('/api/v1/dbms/temp-file-activity', TempFileActivity.fromJson);

  Future<ApiResult<DeadlockInfo>> getDbmsDeadlockHistory() =>
      _get('/api/v1/dbms/deadlock-history', DeadlockInfo.fromJson);

  Future<ApiResult<List<LongTransaction>>> getDbmsLongTransactions() => _get(
    '/api/v1/dbms/long-transactions',
    (json) =>
        (json as List<dynamic>).map((e) => LongTransaction.fromJson(e)).toList(),
  );

  Future<ApiResult<List<IdleInTransactionSession>>>
  getDbmsIdleInTransactionSessions() => _get(
    '/api/v1/dbms/idle-in-transaction-sessions',
    (json) => (json as List<dynamic>)
        .map((e) => IdleInTransactionSession.fromJson(e))
        .toList(),
  );

  /// `resourceKind`/`resourceName` are both-or-neither — the caller
  /// (the suppress dialog) enforces that before this is ever called, so
  /// this never sends a half-built resource to the server.
  Future<ApiResult<void>> suppressDetector({
    required String detectorId,
    String? resourceKind,
    String? resourceName,
    required String reason,
    required String createdBy,
  }) => _postForSuccess(
    '/api/v1/security/suppress',
    jsonEncode({
      'detector_id': detectorId,
      'resource': resourceKind == null
          ? null
          : {'kind': resourceKind, 'name': resourceName},
      'reason': reason,
      'created_by': createdBy,
    }),
  );

  Future<ApiResult<HostVerdict>> getHostAnalysis() =>
      _get('/api/v1/analysis/host', HostVerdict.fromJson);

  /// Unit U45: deliberately called on-demand (a button tap), never
  /// polled — a real AI inference round-trip is expensive, unlike
  /// every other endpoint this client polls on a 1-10s cadence.
  Future<ApiResult<GatedValueForAiExplanation>> getHostExplanation() =>
      _get('/api/v1/analysis/host/explain', GatedValueForAiExplanation.fromJson);

  Future<ApiResult<CorrelationResult>> getCorrelation() =>
      _get('/api/v1/analysis/correlation', CorrelationResult.fromJson);

  /// Unlike grant/reject/suppress, starting a plan returns real,
  /// meaningful data (unit U24) — each candidate's real outcome, which
  /// per ADR 0028 is often not what a caller might assume (e.g.
  /// `AUTO_ALLOWED_PENDING` rather than `PENDING_APPROVAL`) — so this
  /// goes through `_post<T>`, not `_postForSuccess`.
  Future<ApiResult<TuningPlanOutcome>> startTuningPlan({
    required int pid,
    required int startTimeTicks,
    required String resourceName,
    required String profile,
    required String mode,
    required String requestedBy,
  }) => _post(
    '/api/v1/tuning/start',
    jsonEncode({
      'pid': pid,
      'start_time_ticks': startTimeTicks,
      'resource_name': resourceName,
      'profile': profile,
      'mode': mode,
      'requested_by': requestedBy,
    }),
    TuningPlanOutcome.fromJson,
  );

  /// Unit U27: proposing a non-tuning action (e.g. `security.
  /// suspend_process`) directly through the generic propose endpoint
  /// (unit U26) — like `startTuningPlan`, the response carries real,
  /// meaningful data, so this goes through `_post<T>` too. `parameters`
  /// is never sent: no currently-registered non-tuning action type
  /// takes any (`security.suspend_process`'s own validator accepts
  /// only an empty object, which the server already defaults to).
  Future<ApiResult<ActionProposalOutcome>> proposeAction({
    required String actionType,
    required int pid,
    required int startTimeTicks,
    required String resourceName,
    required String requestedBy,
  }) => _post(
    '/api/v1/actions/propose',
    jsonEncode({
      'action_type': actionType,
      'pid': pid,
      'start_time_ticks': startTimeTicks,
      'resource_name': resourceName,
      'requested_by': requestedBy,
    }),
    ActionProposalOutcome.fromJson,
  );

  Future<ApiResult<T>> _get<T>(
    String path,
    T Function(dynamic) dataFromJson,
  ) async {
    final raw = await _transport.getRaw(path);
    return switch (raw) {
      ApiErr(:final failure) => ApiErr(failure),
      ApiOk(:final value) => _decodeEnvelope(value, dataFromJson),
    };
  }

  /// For endpoints whose envelope carries real `data` on success (unlike
  /// `_postForSuccess`'s grant/reject/suppress, which return `()`).
  Future<ApiResult<T>> _post<T>(
    String path,
    String body,
    T Function(dynamic) dataFromJson,
  ) async {
    final raw = await _transport.postRaw(path, body: body);
    return switch (raw) {
      ApiErr(:final failure) => ApiErr(failure),
      ApiOk(:final value) => _decodeEnvelope(value, dataFromJson),
    };
  }

  /// For endpoints whose envelope never carries meaningful `data`, even on
  /// success (grant/reject return `()` on the Rust side, which serializes
  /// as `data: null` regardless of outcome) — `_decodeEnvelope<T>`'s own
  /// "success with null data is malformed" rule doesn't apply here, so
  /// this is a deliberately separate decode path, not a forced reuse.
  Future<ApiResult<void>> _postForSuccess(String path, String body) async {
    final raw = await _transport.postRaw(path, body: body);
    return switch (raw) {
      ApiErr(:final failure) => ApiErr(failure),
      ApiOk(:final value) => _decodeEnvelopeSuccess(value),
    };
  }

  ApiResult<void> _decodeEnvelopeSuccess(String body) {
    final Map<String, dynamic> json;
    try {
      json = jsonDecode(body) as Map<String, dynamic>;
    } on FormatException catch (e) {
      return ApiErr(ApiFailureMalformedPayload('invalid JSON: ${e.message}'));
    } on TypeError {
      return const ApiErr(
        ApiFailureMalformedPayload('response body was not a JSON object'),
      );
    }

    final success = json['success'] as bool?;
    if (success == null) {
      return const ApiErr(
        ApiFailureMalformedPayload('envelope had no "success" field'),
      );
    }
    if (!success) {
      final errorJson = json['error'];
      if (errorJson != null) {
        return ApiErr(ApiFailureServerError(ApiError.fromJson(errorJson)));
      }
      return const ApiErr(
        ApiFailureMalformedPayload('envelope reported failure with no error'),
      );
    }
    return const ApiOk(null);
  }

  ApiResult<T> _decodeEnvelope<T>(
    String body,
    T Function(dynamic) dataFromJson,
  ) {
    final Map<String, dynamic> json;
    try {
      json = jsonDecode(body) as Map<String, dynamic>;
    } on FormatException catch (e) {
      return ApiErr(ApiFailureMalformedPayload('invalid JSON: ${e.message}'));
    } on TypeError {
      return const ApiErr(
        ApiFailureMalformedPayload('response body was not a JSON object'),
      );
    }

    final Envelope<T> envelope;
    try {
      envelope = Envelope.fromJson<T>(json, dataFromJson);
    } catch (e) {
      return ApiErr(ApiFailureMalformedPayload('schema mismatch: $e'));
    }

    if (!envelope.success) {
      if (envelope.error != null) {
        return ApiErr(ApiFailureServerError(envelope.error!));
      }
      return const ApiErr(
        ApiFailureMalformedPayload('envelope reported failure with no error'),
      );
    }
    if (envelope.data == null) {
      return const ApiErr(
        ApiFailureMalformedPayload('envelope reported success with no data'),
      );
    }
    return ApiOk(envelope.data as T);
  }
}
