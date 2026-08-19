// Regression test for a real bug found by a full-codebase scan:
// ConnectionStatusNotifier.retryNow() had no reentrancy guard against the
// periodic timer's own tick. If a slower, earlier-started request (e.g. the
// timer's tick) resolved *after* a faster, later-started one (e.g. a manual
// "retry now" tap), the stale result would unconditionally overwrite the
// fresher state that had already landed — last-completed-wins instead of
// last-started-wins. Fixed with a generation counter; this test exercises
// exactly that ordering using FakeTransport's completer-gated responses to
// control resolution order deterministically (no wall-clock races).

import 'dart:async';
import 'dart:convert';

import 'package:console/api/api_client.dart';
import 'package:console/api/api_result.dart';
import 'package:console/providers/connection_status.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fake_transport.dart';
import 'support/fixtures.dart';

void main() {
  test(
    'a slow tick that resolves after a faster later one does not overwrite it',
    () async {
      final transport = FakeTransport();
      final client = ApiClient(transport);

      final staleHealth = fakeHealth();
      // A distinguishable "version" field so the assertion below can tell
      // whose data actually ended up in state, not just that *some*
      // connected state was reached.
      final staleJson = staleHealth.toJson()..['version'] = '0.0.0-stale';
      final slowGate = Completer<void>();
      transport.queue(
        '/api/v1/health',
        ApiOk(jsonEncode(okEnvelopeJson(staleJson))),
        waitFor: slowGate.future,
      );

      // Constructing the notifier synchronously starts generation 1 (the
      // constructor's own initial tick), which immediately consumes the
      // queued response above and then suspends on `slowGate`.
      final notifier = ConnectionStatusNotifier(client);
      addTearDown(notifier.dispose);

      final freshJson = fakeHealth().toJson()..['version'] = '9.9.9-fresh';
      transport.queue(
        '/api/v1/health',
        ApiOk(jsonEncode(okEnvelopeJson(freshJson))),
      );

      // Generation 2, started after generation 1 but resolves before it
      // (generation 1 is still blocked on slowGate).
      await notifier.retryNow();
      final afterRetry = notifier.state;
      expect(afterRetry, isA<ConnectionConnected>());
      expect((afterRetry as ConnectionConnected).health.version, '9.9.9-fresh');

      // Now let generation 1's stale response finally resolve.
      slowGate.complete();
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      // It must not have overwritten generation 2's already-applied result
      // with the stale generation-1 data.
      final finalState = notifier.state;
      expect(
        finalState,
        isA<ConnectionConnected>(),
        reason: 'a stale, later-resolving tick must not clobber a fresher one',
      );
      expect(
        (finalState as ConnectionConnected).health.version,
        '9.9.9-fresh',
        reason: 'the stale generation-1 response must not have won',
      );
      expect(transport.requestedPaths.length, 2);
    },
  );
}
