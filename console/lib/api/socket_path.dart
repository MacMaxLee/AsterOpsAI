import 'dart:io';

/// Mirrors `service::transport::unix::resolve_socket_path()` on the core
/// side (rust_core/service/src/transport/unix.rs): the socket always lives
/// at `$XDG_RUNTIME_DIR/ai-ops-coordinator/core.sock`, and an unset
/// `XDG_RUNTIME_DIR` is a hard failure, never a silent fallback to `/tmp`
/// (ADR 0001).
String resolveDefaultSocketPath() {
  final runtimeDir = Platform.environment['XDG_RUNTIME_DIR'];
  if (runtimeDir == null || runtimeDir.isEmpty) {
    throw StateError(
      'XDG_RUNTIME_DIR is not set; refusing to guess a socket location.',
    );
  }
  return '$runtimeDir/ai-ops-coordinator/core.sock';
}
