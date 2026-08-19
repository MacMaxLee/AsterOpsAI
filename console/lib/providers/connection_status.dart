import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_client.dart';
import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../api/api_version.dart';
import '../generated/models/health_response.dart';
import 'transport_provider.dart';

sealed class ConnectionStatus {
  const ConnectionStatus();
}

final class ConnectionConnecting extends ConnectionStatus {
  const ConnectionConnecting();
}

final class ConnectionConnected extends ConnectionStatus {
  final HealthResponse health;
  const ConnectionConnected(this.health);
}

/// Distinct from the first-connection failure states below: this fires
/// only after the console has connected at least once, so the banner can
/// stay reassuring ("reconnecting") instead of restating a specific error
/// on every dropped poll.
final class ConnectionReconnecting extends ConnectionStatus {
  final ApiFailure lastFailure;
  const ConnectionReconnecting(this.lastFailure);
}

final class ConnectionUnavailable extends ConnectionStatus {
  final String reason;
  const ConnectionUnavailable(this.reason);
}

final class ConnectionTimeout extends ConnectionStatus {
  const ConnectionTimeout();
}

final class ConnectionMalformedPayload extends ConnectionStatus {
  final String detail;
  const ConnectionMalformedPayload(this.detail);
}

final class ConnectionVersionMismatch extends ConnectionStatus {
  final String coreVersion;
  const ConnectionVersionMismatch(this.coreVersion);
}

const _healthPollInterval = Duration(seconds: 2);

final class ConnectionStatusNotifier extends StateNotifier<ConnectionStatus> {
  final ApiClient _client;
  Timer? _timer;
  bool _everConnected = false;

  ConnectionStatusNotifier(this._client) : super(const ConnectionConnecting()) {
    _tick();
    _timer = Timer.periodic(_healthPollInterval, (_) => _tick());
  }

  /// Lets the UI force an immediate check instead of waiting out the
  /// current poll interval (the "retry now" affordance on error banners).
  Future<void> retryNow() => _tick();

  Future<void> _tick() async {
    final result = await _client.getHealth();
    if (!mounted) return;
    switch (result) {
      case ApiOk(:final value):
        if (value.apiVersion != kSupportedApiVersion) {
          state = ConnectionVersionMismatch(value.apiVersion);
          return;
        }
        _everConnected = true;
        state = ConnectionConnected(value);
      case ApiErr(:final failure):
        if (_everConnected) {
          state = ConnectionReconnecting(failure);
          return;
        }
        state = switch (failure) {
          ApiFailureTimeout() => const ConnectionTimeout(),
          ApiFailureUnavailable(:final reason) => ConnectionUnavailable(reason),
          ApiFailureMalformedPayload(:final detail) =>
            ConnectionMalformedPayload(detail),
          ApiFailureServerError() => const ConnectionUnavailable(
            'core reported an internal error',
          ),
        };
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }
}

final connectionStatusProvider =
    StateNotifierProvider<ConnectionStatusNotifier, ConnectionStatus>((ref) {
      return ConnectionStatusNotifier(ref.watch(apiClientProvider));
    });
