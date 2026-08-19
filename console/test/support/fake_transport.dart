import 'package:console/api/api_failure.dart';
import 'package:console/api/api_result.dart';
import 'package:console/api/local_transport.dart';

/// A scriptable `LocalTransport` for widget/unit tests: no real socket, no
/// real core. Each path gets a strict FIFO queue of responses — one
/// `.queue()` call per expected poll, consumed in order and never repeated
/// — so a test can script "connected, then a mid-session drop, then
/// recovery" deterministically, without a real process to kill.
class FakeTransport implements LocalTransport {
  final Map<String, List<ApiResult<String>>> _queues = {};
  final List<String> requestedPaths = [];

  void queue(String pathPrefix, ApiResult<String> response) {
    _queues.putIfAbsent(pathPrefix, () => []).add(response);
  }

  @override
  Future<ApiResult<String>> getRaw(
    String path, {
    Duration timeout = const Duration(seconds: 5),
  }) async {
    requestedPaths.add(path);
    final prefix = _queues.keys.firstWhere(
      (p) => path.startsWith(p),
      orElse: () => '',
    );
    final queue = _queues[prefix];
    if (queue == null || queue.isEmpty) {
      return const ApiErr(
        ApiFailureUnavailable('FakeTransport: no response scripted'),
      );
    }
    return queue.removeAt(0);
  }

  @override
  void close() {}
}
