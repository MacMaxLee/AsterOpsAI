import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_client.dart';
import '../api/local_transport.dart';
import '../api/socket_path.dart';
import '../api/unix_socket_transport.dart';

/// One transport/client pair for the app's whole lifetime — every
/// repository shares it rather than opening its own connections.
final transportProvider = Provider<LocalTransport>((ref) {
  final transport = UnixSocketTransport(resolveDefaultSocketPath());
  ref.onDispose(transport.close);
  return transport;
});

final apiClientProvider = Provider<ApiClient>((ref) {
  return ApiClient(ref.watch(transportProvider));
});
