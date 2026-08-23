import 'dart:convert';

import 'package:console/api/api_failure.dart';
import 'package:console/api/api_result.dart';
import 'package:console/generated/models/models.dart';
import 'package:console/screens/database_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/fixtures.dart';
import 'support/pump_app.dart';

SessionInfo fakeSession() => SessionInfo(
  clientAddr: '10.0.0.5',
  database: 'appdb',
  pid: 4242,
  query: 'SELECT 1',
  queryStart: DateTime.utc(2026, 1, 1, 9),
  state: SessionState.active,
  username: 'app',
  xactStart: null,
);

LockEdge fakeLock() => const LockEdge(
  blockedPid: 501,
  blockedQuery: 'UPDATE t SET x = 1 WHERE id = 1',
  blockingPid: 500,
  blockingQuery: 'SELECT id FROM t WHERE id = 1 FOR UPDATE',
  lockType: 'transactionid',
);

QueryStat fakeQueryStat() => const QueryStat(
  queryFingerprint: 'abc123',
  normalizedQuery: 'SELECT * FROM t WHERE id = \$1',
  calls: 42,
  totalExecTimeMs: 100.5,
  meanExecTimeMs: 2.39,
  rows: 42,
);

TableStat fakeTableStat() => TableStat(
  schema: 'public',
  table: 'widgets',
  seqScan: 7,
  idxScan: 300,
  nLiveTup: 1500,
  nDeadTup: 12,
  lastVacuum: DateTime.utc(2026, 1, 1, 3),
  lastAutovacuum: null,
  totalSizeBytes: 65536,
);

IndexStat fakeIndexStat() => const IndexStat(
  schema: 'public',
  table: 'widgets',
  index: 'idx_widgets_name',
  idxScan: 300,
  sizeBytes: 8192,
);

ReplicationStatus fakeReplicationStatus({
  List<StandbyInfo> standbys = const [],
}) => ReplicationStatus(isPrimary: true, inRecovery: false, standbys: standbys);

StandbyInfo fakeStandby() => const StandbyInfo(
  clientAddr: '10.0.0.9',
  flushLsn: '0/1A2B3C4',
  replayLagSeconds: 0.85,
  replayLsn: '0/1A2B3C0',
  sentLsn: '0/1A2B3C4',
  state: 'streaming',
  writeLsn: '0/1A2B3C4',
);

GucValue fakeGucValue() => const GucValue(
  name: 'max_connections',
  setting: '100',
  unit: null,
  source: 'configuration file',
);

TempFileActivity fakeTempFileActivity({DateTime? statsReset}) =>
    TempFileActivity(
      tempFiles: 3,
      tempBytes: 2 * 1024 * 1024,
      statsReset: statsReset,
    );

DeadlockInfo fakeDeadlockInfo({DateTime? statsReset}) =>
    DeadlockInfo(deadlocks: 2, statsReset: statsReset);

LongTransaction fakeLongTransaction() => const LongTransaction(
  pid: 7001,
  username: 'app',
  durationSeconds: 125.4,
  state: SessionState.active,
  query: 'UPDATE t SET x = 1',
);

IdleInTransactionSession fakeIdleInTransactionSession() =>
    const IdleInTransactionSession(
      pid: 7002,
      username: 'app',
      idleDurationSeconds: 90.2,
    );

AiExplanation fakeAiExplanation() => const AiExplanation(
  summary: 'The database looks healthy.',
  observations: [
    Observation(text: 'No sustained lock contention.', metrics: []),
  ],
  recommendations: [
    Recommendation(text: 'No action needed.', metrics: [], candidateRef: null),
  ],
  risk: RiskLevel.low,
  confidence: 0.91,
);

Future<void> pumpScreen(WidgetTester tester, dynamic transport) => pumpApp(
  tester,
  const Scaffold(body: DatabaseScreen()),
  transport: transport,
);

void main() {
  testWidgets(
    'a scripted session renders its real pid/username/database/state',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson([fakeSession().toJson()]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('4242'), findsOneWidget);
      expect(find.textContaining('app @ appdb'), findsOneWidget);
      expect(find.textContaining('ACTIVE'), findsOneWidget);
      expect(find.textContaining('SELECT 1'), findsOneWidget);
    },
  );

  testWidgets(
    'a scripted lock renders its real blocked/blocking pids and lock type',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson([fakeLock().toJson()]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('PID 501 blocked by 500'), findsOneWidget);
      expect(find.textContaining('transactionid'), findsOneWidget);
    },
  );

  testWidgets('an empty response for both sections shows the real empty state '
      'independently', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/dbms/sessions',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    transport.queue(
      '/api/v1/dbms/locks',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.text('Nothing to show'), findsNWidgets(2));
  });

  testWidgets(
    'a scripted Supported query-stats response renders the real row',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/query-stats',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForArrayOfQueryStatSupported(value: [fakeQueryStat()])
                  .toJson(),
            ),
          ),
        ),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('SELECT * FROM t'), findsOneWidget);
      expect(find.textContaining('42'), findsWidgets);
      expect(find.textContaining('2.39'), findsOneWidget);
    },
  );

  testWidgets(
    'a scripted Unavailable query-stats response shows the real reason, '
    'not a fabricated generic message',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/query-stats',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              const GatedValueForArrayOfQueryStatUnavailable(
                reason: 'pg_stat_statements extension is not installed',
              ).toJson(),
            ),
          ),
        ),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('Not available'), findsOneWidget);
      expect(
        find.text('pg_stat_statements extension is not installed'),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'a scripted table stat renders its real schema/table/seq_scan/idx_scan/'
    'live/dead tuple fields',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/table-stats',
        ApiOk(jsonEncode(okEnvelopeJson([fakeTableStat().toJson()]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('public.widgets'), findsOneWidget);
      expect(find.textContaining('seq scan: 7'), findsOneWidget);
      expect(find.textContaining('idx scan: 300'), findsOneWidget);
      expect(find.textContaining('live: 1500'), findsOneWidget);
      expect(find.textContaining('dead: 12'), findsOneWidget);
    },
  );

  testWidgets(
    'a scripted index stat renders its real index/table/idx_scan fields',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/index-stats',
        ApiOk(jsonEncode(okEnvelopeJson([fakeIndexStat().toJson()]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('idx_widgets_name'), findsOneWidget);
      expect(find.textContaining('table: widgets'), findsOneWidget);
      expect(find.textContaining('idx scan: 300'), findsOneWidget);
    },
  );

  testWidgets(
    'a scripted replication status renders its real primary/standby summary '
    'and standby row',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/replication',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              fakeReplicationStatus(standbys: [fakeStandby()]).toJson(),
            ),
          ),
        ),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('Primary'), findsOneWidget);
      expect(find.text('10.0.0.9'), findsOneWidget);
      expect(find.textContaining('streaming'), findsOneWidget);
      expect(find.textContaining('0.85'), findsOneWidget);
    },
  );

  testWidgets('a scripted GUC renders its real name/setting/source', (
    tester,
  ) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/dbms/sessions',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    transport.queue(
      '/api/v1/dbms/locks',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    transport.queue(
      '/api/v1/dbms/gucs',
      ApiOk(jsonEncode(okEnvelopeJson([fakeGucValue().toJson()]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.text('max_connections'), findsOneWidget);
    expect(find.textContaining('100'), findsOneWidget);
    expect(find.textContaining('configuration file'), findsOneWidget);
  });

  testWidgets(
    'a scripted temp file activity renders its real file count/bytes and a '
    'real non-null stats_reset',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/temp-file-activity',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              fakeTempFileActivity(statsReset: DateTime.utc(2026, 1, 1, 9))
                  .toJson(),
            ),
          ),
        ),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('3 temp files'), findsOneWidget);
      expect(find.textContaining('2.0 MB'), findsOneWidget);
      expect(find.textContaining('stats reset:'), findsOneWidget);
    },
  );

  testWidgets(
    'a scripted deadlock history renders its real count and never shows a '
    'fabricated stats_reset when the real value is null',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/deadlock-history',
        ApiOk(jsonEncode(okEnvelopeJson(fakeDeadlockInfo().toJson()))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('2 deadlock(s)'), findsOneWidget);
      expect(find.textContaining('stats reset:'), findsNothing);
    },
  );

  testWidgets('a scripted long transaction renders its real pid/username/state/'
      'duration/query', (tester) async {
    final transport = createFakeTransport();
    transport.queue(
      '/api/v1/dbms/sessions',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    transport.queue(
      '/api/v1/dbms/locks',
      ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
    );
    transport.queue(
      '/api/v1/dbms/long-transactions',
      ApiOk(jsonEncode(okEnvelopeJson([fakeLongTransaction().toJson()]))),
    );
    await pumpScreen(tester, transport);
    await tester.pump();

    expect(find.textContaining('7001'), findsOneWidget);
    expect(find.textContaining('app'), findsWidgets);
    expect(find.textContaining('ACTIVE'), findsOneWidget);
    expect(find.textContaining('125.4'), findsOneWidget);
    expect(find.textContaining('UPDATE t SET x = 1'), findsOneWidget);
  });

  testWidgets(
    'a scripted idle-in-transaction session renders its real pid/username/'
    'idle duration',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/idle-in-transaction-sessions',
        ApiOk(
          jsonEncode(okEnvelopeJson([fakeIdleInTransactionSession().toJson()])),
        ),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('7002'), findsOneWidget);
      expect(find.textContaining('app'), findsWidgets);
      expect(find.textContaining('90.2'), findsOneWidget);
    },
  );

  testWidgets(
    'tapping Explain with AI fires the real DB explain request and renders '
    'a real Supported explanation',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/db/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForAiExplanationSupported(value: fakeAiExplanation())
                  .toJson(),
            ),
          ),
        ),
      );
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('Explain with AI'), findsOneWidget);
      // `DatabaseScreen` genuinely no longer fits an 800x600 test
      // viewport (ADR 0047's own overflow lesson) — `tester.tap()`
      // doesn't auto-scroll a target into view, so this section's own
      // button must be scrolled to first, the same real requirement a
      // production user would face on a small window.
      await tester.ensureVisible(find.text('Explain with AI'));
      await tester.tap(find.text('Explain with AI'));
      await tester.pump();
      await tester.pump();

      expect(find.text('The database looks healthy.'), findsOneWidget);
      expect(find.text('•  No sustained lock contention.'), findsOneWidget);
      expect(find.text('→  No action needed.'), findsOneWidget);
      expect(find.textContaining('LOW'), findsOneWidget);
      expect(find.textContaining('91%'), findsOneWidget);
    },
  );

  testWidgets(
    'a real Unavailable gated DB explanation shows the real reason via the '
    'shared vocabulary',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/db/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              const GatedValueForAiExplanationUnavailable(
                reason: 'AI explanation unavailable',
              ).toJson(),
            ),
          ),
        ),
      );
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      // `DatabaseScreen` genuinely no longer fits an 800x600 test
      // viewport (ADR 0047's own overflow lesson) — `tester.tap()`
      // doesn't auto-scroll a target into view, so this section's own
      // button must be scrolled to first, the same real requirement a
      // production user would face on a small window.
      await tester.ensureVisible(find.text('Explain with AI'));
      await tester.tap(find.text('Explain with AI'));
      await tester.pump();
      await tester.pump();

      expect(find.text('Not available'), findsOneWidget);
      expect(find.text('AI explanation unavailable'), findsOneWidget);
    },
  );

  testWidgets(
    'a real transport-level error on DB explain shows the real failure '
    'message with a working retry',
    (tester) async {
      final transport = createFakeTransport();
      transport.queue(
        '/api/v1/analysis/db/explain',
        const ApiErr(ApiFailureUnavailable('connection refused')),
      );
      transport.queue(
        '/api/v1/dbms/sessions',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      transport.queue(
        '/api/v1/dbms/locks',
        ApiOk(jsonEncode(okEnvelopeJson(<dynamic>[]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      // `DatabaseScreen` genuinely no longer fits an 800x600 test
      // viewport (ADR 0047's own overflow lesson) — `tester.tap()`
      // doesn't auto-scroll a target into view, so this section's own
      // button must be scrolled to first, the same real requirement a
      // production user would face on a small window.
      await tester.ensureVisible(find.text('Explain with AI'));
      await tester.tap(find.text('Explain with AI'));
      await tester.pump();
      await tester.pump();

      // Keyed, not `find.text(...)`: every other (independently-polled)
      // DBMS zone on this screen also renders this exact generic
      // wording for its own unrelated, genuinely-unscripted failure —
      // the `Key` targets this section's own message precisely.
      expect(
        find.byWidgetPredicate(
          (widget) =>
              widget.key == const Key('dbExplanationError') &&
              widget is Text &&
              widget.data ==
                  "The AsterOpsAI core service isn't reachable. It may "
                      'have stopped, or the socket may be missing. The '
                      'console will keep retrying automatically.',
        ),
        findsOneWidget,
      );

      transport.queue(
        '/api/v1/analysis/db/explain',
        ApiOk(
          jsonEncode(
            okEnvelopeJson(
              GatedValueForAiExplanationSupported(value: fakeAiExplanation())
                  .toJson(),
            ),
          ),
        ),
      );
      // `DatabaseScreen` genuinely no longer fits an 800x600 test
      // viewport (ADR 0047's own overflow lesson) — `tester.tap()`
      // doesn't auto-scroll a target into view, so this section's own
      // button must be scrolled to first, the same real requirement a
      // production user would face on a small window.
      await tester.ensureVisible(find.text('Explain with AI'));
      await tester.tap(find.text('Explain with AI'));
      await tester.pump();
      await tester.pump();

      expect(find.text('The database looks healthy.'), findsOneWidget);
    },
  );
}
