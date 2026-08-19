import '../generated/models/api_error.dart';

/// Every way a console API call can fail, closed so every call site is
/// forced to handle each distinctly (SRS FR-CONSOLE-002 / U3 requirement 4)
/// instead of falling back to one generic "error" state.
sealed class ApiFailure {
  const ApiFailure();
}

/// The request round-trip (connect, send, or read) exceeded its deadline.
final class ApiFailureTimeout extends ApiFailure {
  const ApiFailureTimeout();
}

/// The core isn't reachable at all — socket missing, connection refused,
/// or the service process isn't running.
final class ApiFailureUnavailable extends ApiFailure {
  final String reason;
  const ApiFailureUnavailable(this.reason);
}

/// The core responded, but the body wasn't valid JSON or didn't match the
/// expected schema shape.
final class ApiFailureMalformedPayload extends ApiFailure {
  final String detail;
  const ApiFailureMalformedPayload(this.detail);
}

/// The core responded with a well-formed envelope whose `success` was
/// false — a real, closed `ApiError` came back.
final class ApiFailureServerError extends ApiFailure {
  final ApiError error;
  const ApiFailureServerError(this.error);
}
