import 'dart:async';

import '../api/api_result.dart';

/// The repository layer TRS §15 calls for: owns the polling loop for one
/// data family, exposes a stream of results, and nothing else — no
/// thresholds or reclassification of what the API client hands it. Screens
/// only ever see this through a provider (lib/providers/), never directly.
final class PolledRepository<T> {
  final Future<ApiResult<T>> Function() _fetch;
  final Duration _interval;
  final _controller = StreamController<ApiResult<T>>.broadcast();
  Timer? _timer;

  PolledRepository({
    required Future<ApiResult<T>> Function() fetch,
    required Duration interval,
  })  : _fetch = fetch,
        _interval = interval {
    _tick();
    _timer = Timer.periodic(_interval, (_) => _tick());
  }

  Stream<ApiResult<T>> get stream => _controller.stream;

  Future<void> _tick() async {
    if (_controller.isClosed) return;
    final result = await _fetch();
    if (!_controller.isClosed) {
      _controller.add(result);
    }
  }

  void dispose() {
    _timer?.cancel();
    _controller.close();
  }
}
