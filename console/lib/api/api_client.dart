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
