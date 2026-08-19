import '../generated/models/api_error.dart';

/// Every core API response is wrapped in this shape:
/// `{success, timestamp, request_id, data, error}`. Hand-written, not
/// generated — every `envelope_*.schema.json` is the same structure
/// monomorphized per data type, exactly like `contracts::Envelope<T>` on the
/// Rust side is itself hand-written and only gets monomorphized away when
/// schemas are *emitted*. See tool/generate_models.dart's header comment.
final class Envelope<T> {
  final bool success;
  final DateTime timestamp;
  final String requestId;
  final T? data;
  final ApiError? error;

  const Envelope({
    required this.success,
    required this.timestamp,
    required this.requestId,
    required this.data,
    required this.error,
  });

  static Envelope<T> fromJson<T>(
    Map<String, dynamic> json,
    T Function(dynamic) dataFromJson,
  ) {
    final rawData = json['data'];
    return Envelope<T>(
      success: json['success'] as bool,
      timestamp: DateTime.parse(json['timestamp'] as String),
      requestId: json['request_id'] as String,
      data: rawData == null ? null : dataFromJson(rawData),
      error: json['error'] == null ? null : ApiError.fromJson(json['error']),
    );
  }
}
