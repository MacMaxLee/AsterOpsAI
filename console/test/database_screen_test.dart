import 'dart:convert';

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

ReplicationStatus fakeReplicationStatus({List<StandbyInfo> standbys = const []}) =>
    ReplicationStatus(
      isPrimary: true,
      inRecovery: false,
      standbys: standbys,
    );

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

  testWidgets(
    'a scripted GUC renders its real name/setting/source',
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
        '/api/v1/dbms/gucs',
        ApiOk(jsonEncode(okEnvelopeJson([fakeGucValue().toJson()]))),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.text('max_connections'), findsOneWidget);
      expect(find.textContaining('100'), findsOneWidget);
      expect(find.textContaining('configuration file'), findsOneWidget);
    },
  );

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
              fakeTempFileActivity(
                statsReset: DateTime.utc(2026, 1, 1, 9),
              ).toJson(),
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
        ApiOk(
          jsonEncode(okEnvelopeJson(fakeDeadlockInfo().toJson())),
        ),
      );
      await pumpScreen(tester, transport);
      await tester.pump();

      expect(find.textContaining('2 deadlock(s)'), findsOneWidget);
      expect(find.textContaining('stats reset:'), findsNothing);
    },
  );
}
