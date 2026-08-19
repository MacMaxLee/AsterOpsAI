import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'api_failure.dart';
import 'api_result.dart';
import 'local_transport.dart';

/// Speaks HTTP to the core over a Unix domain socket. `dart:io`'s
/// `HttpClient` only opens real TCP connections by default; the documented
/// escape hatch is `connectionFactory`, which lets every connection this
/// client makes go through `Socket.startConnect` against a Unix-domain
/// `InternetAddress` instead — the rest of `HttpClient`'s request/response
/// handling (headers, chunked bodies, etc.) is reused unmodified. The host
/// in every request URI is a placeholder; axum doesn't route on it and the
/// actual byte stream never touches a real network interface.
final class UnixSocketTransport implements LocalTransport {
  final String socketPath;
  late final HttpClient _client;

  UnixSocketTransport(this.socketPath) {
    _client = HttpClient()
      ..connectionFactory = (uri, proxyHost, proxyPort) {
        return Socket.startConnect(
          InternetAddress(socketPath, type: InternetAddressType.unix),
          0,
        );
      };
  }

  @override
  Future<ApiResult<String>> getRaw(
    String path, {
    Duration timeout = const Duration(seconds: 5),
  }) async {
    try {
      final request = await _client
          .getUrl(Uri.http('localhost', path))
          .timeout(timeout);
      final response = await request.close().timeout(timeout);
      final body = await response
          .transform(utf8.decoder)
          .join()
          .timeout(timeout);
      return ApiOk(body);
    } on TimeoutException {
      return const ApiErr(ApiFailureTimeout());
    } on SocketException catch (e) {
      return ApiErr(ApiFailureUnavailable(e.message));
    } on HttpException catch (e) {
      return ApiErr(ApiFailureUnavailable(e.message));
    }
  }

  @override
  void close() => _client.close(force: true);
}
