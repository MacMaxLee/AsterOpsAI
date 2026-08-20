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
      }
      // Bounds the connect phase specifically (a hung/black-holed socket
      // connect, as opposed to a slow response after connecting), so a
      // fully wedged core can't hold this open indefinitely even before
      // the per-call `getRaw` timeout below has a request to time out.
      ..connectionTimeout = const Duration(seconds: 5);
  }

  @override
  Future<ApiResult<String>> getRaw(
    String path, {
    Duration timeout = const Duration(seconds: 5),
  }) async {
    try {
      // One timeout around the whole request/response sequence, not one
      // per stage — three independently-timed `.timeout()` calls (connect,
      // response headers, body read) could each wait up to `timeout` before
      // firing, so a single slow-but-not-dead core could take up to 3x
      // `timeout` to actually report `ApiFailureTimeout`, not `timeout`
      // itself. Dart's `Future.timeout()` still can't cancel the
      // in-progress work if it eventually completes after this fires — no
      // true cancellation exists for an arbitrary async chain in Dart — so
      // `connectionTimeout` above and `HttpClient`'s own default
      // `idleTimeout` (15s) are what actually bound a wedged connection's
      // lifetime, not this alone.
      final body = await _fetchBody(path).timeout(timeout);
      return ApiOk(body);
    } on TimeoutException {
      return const ApiErr(ApiFailureTimeout());
    } on SocketException catch (e) {
      return ApiErr(ApiFailureUnavailable(e.message));
    } on HttpException catch (e) {
      return ApiErr(ApiFailureUnavailable(e.message));
    } catch (e) {
      // Anything else (e.g. the client having been closed mid-request
      // during app/provider teardown) is still a real, reportable failure,
      // not something that should propagate as an unhandled exception out
      // of a background poll loop.
      return ApiErr(ApiFailureUnavailable(e.toString()));
    }
  }

  Future<String> _fetchBody(String path) async {
    final request = await _client.getUrl(Uri.http('localhost', path));
    final response = await request.close();
    return response.transform(utf8.decoder).join();
  }

  @override
  Future<ApiResult<String>> postRaw(
    String path, {
    required String body,
    Duration timeout = const Duration(seconds: 5),
  }) async {
    try {
      final responseBody = await _postBody(path, body).timeout(timeout);
      return ApiOk(responseBody);
    } on TimeoutException {
      return const ApiErr(ApiFailureTimeout());
    } on SocketException catch (e) {
      return ApiErr(ApiFailureUnavailable(e.message));
    } on HttpException catch (e) {
      return ApiErr(ApiFailureUnavailable(e.message));
    } catch (e) {
      return ApiErr(ApiFailureUnavailable(e.toString()));
    }
  }

  Future<String> _postBody(String path, String body) async {
    final request = await _client.postUrl(Uri.http('localhost', path));
    request.headers.contentType = ContentType.json;
    request.write(body);
    final response = await request.close();
    return response.transform(utf8.decoder).join();
  }

  @override
  void close() => _client.close(force: true);
}
