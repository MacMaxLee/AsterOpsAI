import 'api_result.dart';

/// A transport that can reach the core's local API. ADR 0001 rules out a
/// TCP loopback base URL entirely — every implementation must speak
/// whatever the platform's local IPC primitive is (Unix domain socket on
/// Linux/macOS today; a Windows named pipe implementation is U12's job,
/// behind this same interface).
abstract class LocalTransport {
  Future<ApiResult<String>> getRaw(
    String path, {
    Duration timeout = const Duration(seconds: 5),
  });

  void close();
}
