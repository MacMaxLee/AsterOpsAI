// Live E2E check, not a mock: starts the real `ai-ops-core` binary, drives
// the console's real transport/API-client layer against its real Unix
// domain socket, kills the process mid-session, and confirms the client
// notices and then recovers once the process comes back — all without any
// Flutter widget tree (no GTK/desktop-embedder toolchain needed to run
// this, see U3's plan for why that matters on this dev box). Run:
//
//   dart run integration/reconnect_live_test.dart
//
// Exits 0 on success, prints a diagnosis and exits 1 on the first check
// that fails.
//
// This is a CLI diagnostic script, not library code — printing progress is
// its entire job (the same reasoning `service::main.rs`'s `audit verify`
// subcommand applies to its own `println!`s).
// ignore_for_file: avoid_print

import 'dart:io';

import 'package:console/api/api_client.dart';
import 'package:console/api/api_result.dart';
import 'package:console/api/unix_socket_transport.dart';
import 'package:console/generated/models/health_response.dart';

Future<void> main() async {
  final repoRoot = _findRepoRoot();
  final binaryPath = '${repoRoot.path}/target/debug/ai-ops-core';
  if (!File(binaryPath).existsSync()) {
    stderr.writeln(
      'error: $binaryPath not found — run `cargo build --workspace` first',
    );
    exit(2);
  }

  final tmp = Directory.systemTemp.createTempSync('aoa-console-e2e-');
  final runtimeDir = Directory('${tmp.path}/run')..createSync();
  final dataDir = Directory('${tmp.path}/data')..createSync();
  final socketPath = '${runtimeDir.path}/ai-ops-coordinator/core.sock';

  final env = {
    'XDG_RUNTIME_DIR': runtimeDir.path,
    'XDG_DATA_HOME': dataDir.path,
  };

  try {
    print('[1/5] starting ai-ops-core...');
    var process = await Process.start(
      binaryPath,
      ['serve'],
      environment: env,
      includeParentEnvironment: true,
    );
    _forwardStderr(process, label: 'core');
    await _waitForSocket(socketPath);

    final transport = UnixSocketTransport(socketPath);
    final client = ApiClient(transport);

    print('[2/5] confirming the client can reach a freshly-started core...');
    final firstHealth = await _pollUntilOk(client);
    if (firstHealth == null) {
      stderr.writeln('FAILED: client never reached a healthy state');
      exit(1);
    }
    print('    ok: api_version=${firstHealth.apiVersion}');

    print('[3/5] killing the core mid-session...');
    process.kill(ProcessSignal.sigterm);
    await process.exitCode;

    print('[4/5] confirming the client notices the outage...');
    final sawFailure = await _pollUntilFailure(client);
    if (!sawFailure) {
      stderr.writeln(
        'FAILED: client still reports success after the core was killed',
      );
      exit(1);
    }
    print('    ok: client reports failure while the core is down');

    print('[5/5] restarting the core and confirming automatic recovery...');
    process = await Process.start(
      binaryPath,
      ['serve'],
      environment: env,
      includeParentEnvironment: true,
    );
    _forwardStderr(process, label: 'core (restarted)');
    await _waitForSocket(socketPath);

    final recovered = await _pollUntilOk(client);
    if (recovered == null) {
      stderr.writeln(
        'FAILED: client did not recover after the core restarted — no client-side restart was performed',
      );
      exit(1);
    }
    print('    ok: client recovered on its own, no restart of the client');

    transport.close();
    process.kill(ProcessSignal.sigterm);
    await process.exitCode;

    print('\nOK: live reconnect check passed.');
  } finally {
    tmp.deleteSync(recursive: true);
  }
}

void _forwardStderr(Process process, {required String label}) {
  process.stderr
      .transform(SystemEncoding().decoder)
      .listen((line) => stderr.write('[$label] $line'));
}

Future<void> _waitForSocket(String socketPath) async {
  final deadline = DateTime.now().add(const Duration(seconds: 10));
  while (DateTime.now().isBefore(deadline)) {
    if (File(socketPath).existsSync()) return;
    await Future.delayed(const Duration(milliseconds: 100));
  }
  stderr.writeln('error: socket $socketPath never appeared');
  exit(2);
}

/// Polls until a call succeeds, or gives up after a real deadline —
/// distinguishing "still starting up" from "actually broken."
Future<HealthResponse?> _pollUntilOk(ApiClient client) async {
  final deadline = DateTime.now().add(const Duration(seconds: 10));
  while (DateTime.now().isBefore(deadline)) {
    final result = await client.getHealth();
    if (result case ApiOk(:final value)) return value;
    await Future.delayed(const Duration(milliseconds: 200));
  }
  return null;
}

Future<bool> _pollUntilFailure(ApiClient client) async {
  final deadline = DateTime.now().add(const Duration(seconds: 10));
  while (DateTime.now().isBefore(deadline)) {
    final result = await client.getHealth();
    if (result is ApiErr) return true;
    await Future.delayed(const Duration(milliseconds: 200));
  }
  return false;
}

Directory _findRepoRoot() {
  var dir = Directory.current;
  while (true) {
    if (File('${dir.path}/Cargo.toml').existsSync() &&
        Directory('${dir.path}/rust_core').existsSync()) {
      return dir;
    }
    final parent = dir.parent;
    if (parent.path == dir.path) {
      stderr.writeln(
        'error: could not find the repo root above ${Directory.current.path}',
      );
      exit(2);
    }
    dir = parent;
  }
}
