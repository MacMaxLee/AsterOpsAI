import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../generated/models/ai_explanation.dart';
import '../generated/models/deadlock_info.dart';
import '../generated/models/gated_value_for_array_of_query_stat.dart';
import '../generated/models/gated_value_for_ai_explanation.dart';
import '../generated/models/guc_value.dart';
import '../generated/models/idle_in_transaction_session.dart';
import '../generated/models/index_stat.dart';
import '../generated/models/lock_edge.dart';
import '../generated/models/long_transaction.dart';
import '../generated/models/query_stat.dart';
import '../generated/models/replication_status.dart';
import '../generated/models/session_info.dart';
import '../generated/models/standby_info.dart';
import '../generated/models/table_stat.dart';
import '../generated/models/temp_file_activity.dart';
import '../l10n/app_localizations.dart';
import '../providers/dbms_providers.dart';
import '../providers/transport_provider.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';

/// Unit U32/U34/U36/U38/U40/U42: the console surface for unit
/// U31/U33/U35/U37/U39/U41's own direct DBMS endpoints — the last pair
/// (`Long transactions`, `Idle in transaction`) completes the console
/// side of the whole DBMS wiring vein (ADR 0046/0047). Independent
/// sections, each watching its own provider, so a slow/failed poll on
/// one never blocks the others — the same shape `DashboardScreen`'s
/// own multiple independently-polled `AsyncResultView` sections already
/// use. Every endpoint here is read-only; there is no mutation UI.
class DatabaseScreen extends ConsumerWidget {
  const DatabaseScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final sessions = ref.watch(dbmsSessionsProvider);
    final locks = ref.watch(dbmsLocksProvider);
    final queryStats = ref.watch(dbmsQueryStatsProvider);
    final tableStats = ref.watch(dbmsTableStatsProvider);
    final indexStats = ref.watch(dbmsIndexStatsProvider);
    final replication = ref.watch(dbmsReplicationProvider);
    final gucs = ref.watch(dbmsGucsProvider);
    final tempFileActivity = ref.watch(dbmsTempFileActivityProvider);
    final deadlockHistory = ref.watch(dbmsDeadlockHistoryProvider);
    final longTransactions = ref.watch(dbmsLongTransactionsProvider);
    final idleInTransactionSessions = ref.watch(
      dbmsIdleInTransactionSessionsProvider,
    );

    // A scrollable `Column` (not a fixed-height `Expanded` split) — six
    // zones no longer reliably fit a typical window without scrolling.
    // `SingleChildScrollView` (unlike `ListView`'s lazy sliver children)
    // still fully builds every zone eagerly, so every section stays
    // immediately findable in tests without a scroll gesture first. Each
    // zone gets a fixed height so its own `ListView.builder` has the
    // bounded height it needs.
    const zoneHeight = 240.0;
    return SingleChildScrollView(
      child: Column(
        children: [
          SizedBox(
            height: zoneHeight,
            child: Row(
              children: [
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionSessions,
                    child: AsyncResultView<List<SessionInfo>>(
                      asyncValue: sessions,
                      builder: (context, list) => _SessionsList(sessions: list),
                    ),
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionLocks,
                    child: AsyncResultView<List<LockEdge>>(
                      asyncValue: locks,
                      builder: (context, list) => _LocksList(locks: list),
                    ),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          SizedBox(
            height: zoneHeight,
            child: _Section(
              title: l10n.dbmsSectionQueryStats,
              child: AsyncResultView<GatedValueForArrayOfQueryStat>(
                asyncValue: queryStats,
                builder: (context, gated) => _QueryStatsSection(gated: gated),
              ),
            ),
          ),
          const Divider(height: 1),
          SizedBox(
            height: zoneHeight,
            child: Row(
              children: [
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionTableStats,
                    child: AsyncResultView<List<TableStat>>(
                      asyncValue: tableStats,
                      builder: (context, list) => _TableStatsList(stats: list),
                    ),
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionIndexStats,
                    child: AsyncResultView<List<IndexStat>>(
                      asyncValue: indexStats,
                      builder: (context, list) => _IndexStatsList(stats: list),
                    ),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          SizedBox(
            height: zoneHeight,
            child: Row(
              children: [
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionReplication,
                    child: AsyncResultView<ReplicationStatus>(
                      asyncValue: replication,
                      builder: (context, status) =>
                          _ReplicationSection(status: status),
                    ),
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionGucs,
                    child: AsyncResultView<List<GucValue>>(
                      asyncValue: gucs,
                      builder: (context, list) => _GucsList(gucs: list),
                    ),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          SizedBox(
            height: zoneHeight,
            child: Row(
              children: [
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionTempFileActivity,
                    child: AsyncResultView<TempFileActivity>(
                      asyncValue: tempFileActivity,
                      builder: (context, activity) =>
                          _TempFileActivitySection(activity: activity),
                    ),
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionDeadlockHistory,
                    child: AsyncResultView<DeadlockInfo>(
                      asyncValue: deadlockHistory,
                      builder: (context, info) =>
                          _DeadlockHistorySection(info: info),
                    ),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          SizedBox(
            height: zoneHeight,
            child: Row(
              children: [
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionLongTransactions,
                    child: AsyncResultView<List<LongTransaction>>(
                      asyncValue: longTransactions,
                      builder: (context, list) =>
                          _LongTransactionsList(transactions: list),
                    ),
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: _Section(
                    title: l10n.dbmsSectionIdleInTransactionSessions,
                    child: AsyncResultView<List<IdleInTransactionSession>>(
                      asyncValue: idleInTransactionSessions,
                      builder: (context, list) =>
                          _IdleInTransactionSessionsList(sessions: list),
                    ),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 32),
          // Unit U47: no `zoneHeight` `SizedBox` here on purpose — unlike
          // every zone above, this section has no internal
          // `ListView.builder` needing a bounded height (mirrors
          // `host_analysis_screen.dart`'s own unconstrained
          // `_ExplanationSection`, ADR 0050).
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16),
            child: _ExplanationSection(),
          ),
        ],
      ),
    );
  }
}

class _Section extends StatelessWidget {
  final String title;
  final Widget child;
  const _Section({required this.title, required this.child});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(8),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(title, style: Theme.of(context).textTheme.titleMedium),
          ),
        ),
        Expanded(child: child),
      ],
    );
  }
}

class _SessionsList extends StatelessWidget {
  final List<SessionInfo> sessions;
  const _SessionsList({required this.sessions});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (sessions.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: sessions.length,
      itemBuilder: (context, index) => _SessionRow(session: sessions[index]),
    );
  }
}

class _SessionRow extends StatelessWidget {
  final SessionInfo session;
  const _SessionRow({required this.session});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text('${l10n.dbmsColumnPid}: ${session.pid}'),
      subtitle: Text(
        [
          l10n.dbmsSessionRowSubtitle(
            session.username ?? '?',
            session.database ?? '?',
          ),
          '${l10n.dbmsColumnState}: ${session.state.wireValue}',
          if (session.query != null) session.query!,
        ].join('  •  '),
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}

class _LocksList extends StatelessWidget {
  final List<LockEdge> locks;
  const _LocksList({required this.locks});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (locks.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: locks.length,
      itemBuilder: (context, index) => _LockRow(lock: locks[index]),
    );
  }
}

class _LockRow extends StatelessWidget {
  final LockEdge lock;
  const _LockRow({required this.lock});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      title: Text(_titleFor(context, lock)),
      subtitle: Text(
        [
          lock.lockType,
          if (lock.blockedQuery != null) lock.blockedQuery!,
        ].join('  •  '),
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }

  String _titleFor(BuildContext context, LockEdge lock) {
    final l10n = AppLocalizations.of(context)!;
    return l10n.dbmsLockRowTitle(lock.blockedPid, lock.blockingPid);
  }
}

/// Unlike `_SessionsList`/`_LocksList` (plain lists), this endpoint's
/// data is wrapped in `GatedValueForArrayOfQueryStat` — `pg_stat_
/// statements` may genuinely not be installed. The non-`Supported`
/// states reuse `metric_display.dart`'s own existing icon/l10n
/// vocabulary (`metricStateUnavailableTitle`/`metricStateLimitedTitle`/
/// `metricStatePermissionRequiredTitle`) rather than inventing a second
/// one — that widget itself can't be reused directly here since it
/// renders one inline value, not a whole list-or-message section.
class _QueryStatsSection extends StatelessWidget {
  final GatedValueForArrayOfQueryStat gated;
  const _QueryStatsSection({required this.gated});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return switch (gated) {
      GatedValueForArrayOfQueryStatSupported(:final value) => _QueryStatsList(
        stats: value,
      ),
      GatedValueForArrayOfQueryStatLimited(:final reason) => _GatedMessage(
        icon: Icons.info_outline,
        title: l10n.metricStateLimitedTitle,
        reason: reason,
      ),
      GatedValueForArrayOfQueryStatUnavailable(:final reason) => _GatedMessage(
        icon: Icons.remove_circle_outline,
        title: l10n.metricStateUnavailableTitle,
        reason: reason,
      ),
      GatedValueForArrayOfQueryStatPermissionRequired(:final reason) =>
        _GatedMessage(
          icon: Icons.lock_outline,
          title: l10n.metricStatePermissionRequiredTitle,
          reason: reason,
        ),
    };
  }
}

class _GatedMessage extends StatelessWidget {
  final IconData icon;
  final String title;
  final String reason;
  const _GatedMessage({
    required this.icon,
    required this.title,
    required this.reason,
  });

  @override
  Widget build(BuildContext context) {
    final muted = Theme.of(context).colorScheme.onSurfaceVariant;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, color: muted),
          const SizedBox(height: 8),
          Text(title, style: TextStyle(color: muted)),
          Text(reason, style: TextStyle(color: muted)),
        ],
      ),
    );
  }
}

class _QueryStatsList extends StatelessWidget {
  final List<QueryStat> stats;
  const _QueryStatsList({required this.stats});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (stats.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: stats.length,
      itemBuilder: (context, index) => _QueryStatRow(stat: stats[index]),
    );
  }
}

class _QueryStatRow extends StatelessWidget {
  final QueryStat stat;
  const _QueryStatRow({required this.stat});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text(
        stat.normalizedQuery,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        l10n.dbmsQueryStatRowSubtitle(
          stat.calls,
          stat.meanExecTimeMs.toStringAsFixed(2),
          stat.rows,
        ),
      ),
    );
  }
}

class _TableStatsList extends StatelessWidget {
  final List<TableStat> stats;
  const _TableStatsList({required this.stats});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (stats.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: stats.length,
      itemBuilder: (context, index) => _TableStatRow(stat: stats[index]),
    );
  }
}

class _TableStatRow extends StatelessWidget {
  final TableStat stat;
  const _TableStatRow({required this.stat});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text('${stat.schema}.${stat.table}'),
      subtitle: Text(
        l10n.dbmsTableStatRowSubtitle(
          stat.seqScan,
          stat.idxScan,
          stat.nLiveTup,
          stat.nDeadTup,
        ),
      ),
    );
  }
}

class _IndexStatsList extends StatelessWidget {
  final List<IndexStat> stats;
  const _IndexStatsList({required this.stats});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (stats.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: stats.length,
      itemBuilder: (context, index) => _IndexStatRow(stat: stats[index]),
    );
  }
}

class _IndexStatRow extends StatelessWidget {
  final IndexStat stat;
  const _IndexStatRow({required this.stat});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text(stat.index),
      subtitle: Text(l10n.dbmsIndexStatRowSubtitle(stat.table, stat.idxScan)),
    );
  }
}

/// Unlike every other section here (a flat list), `ReplicationStatus`
/// is a summary (`is_primary`/`in_recovery`) plus a nested `standbys`
/// list — a summary row above a divider, then the real standby list
/// below. No derived "is replication healthy" judgment is computed
/// here (unit U36's own precedent) — the real state and real standby
/// fields are shown as-is.
class _ReplicationSection extends StatelessWidget {
  final ReplicationStatus status;
  const _ReplicationSection({required this.status});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Column(
      children: [
        ListTile(
          title: Text(
            status.isPrimary
                ? l10n.dbmsReplicationPrimary
                : l10n.dbmsReplicationStandby,
          ),
        ),
        const Divider(height: 1),
        Expanded(child: _StandbysList(standbys: status.standbys)),
      ],
    );
  }
}

class _StandbysList extends StatelessWidget {
  final List<StandbyInfo> standbys;
  const _StandbysList({required this.standbys});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (standbys.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: standbys.length,
      itemBuilder: (context, index) => _StandbyRow(standby: standbys[index]),
    );
  }
}

class _StandbyRow extends StatelessWidget {
  final StandbyInfo standby;
  const _StandbyRow({required this.standby});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final lag = standby.replayLagSeconds;
    return ListTile(
      title: Text(standby.clientAddr ?? '?'),
      subtitle: Text(
        l10n.dbmsStandbyRowSubtitle(
          standby.state,
          lag == null ? '?' : lag.toStringAsFixed(2),
        ),
      ),
    );
  }
}

class _GucsList extends StatelessWidget {
  final List<GucValue> gucs;
  const _GucsList({required this.gucs});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (gucs.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: gucs.length,
      itemBuilder: (context, index) => _GucRow(guc: gucs[index]),
    );
  }
}

class _GucRow extends StatelessWidget {
  final GucValue guc;
  const _GucRow({required this.guc});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final settingWithUnit = guc.unit == null
        ? guc.setting
        : '${guc.setting}${guc.unit}';
    return ListTile(
      title: Text(guc.name),
      subtitle: Text(l10n.dbmsGucRowSubtitle(settingWithUnit, guc.source)),
    );
  }
}

/// Unlike every other section here, `TempFileActivity`/`DeadlockInfo`
/// are single-object flat summaries with no collection at all — a
/// small `Column` of labeled lines, not a `ListView`. `stats_reset` is
/// shown only when non-null (matching `tuning_history_screen.dart`'s
/// own "never fabricate a completion time" precedent for its own
/// nullable `completedAt`) — a null reset time is real information
/// (PostgreSQL just hasn't reset these counters since the cluster
/// started), not an error to paper over with a placeholder.
class _TempFileActivitySection extends StatelessWidget {
  final TempFileActivity activity;
  const _TempFileActivitySection({required this.activity});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final statsReset = activity.statsReset;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            l10n.dbmsTempFileActivitySummary(
              activity.tempFiles,
              formatBytes(activity.tempBytes),
            ),
          ),
          if (statsReset != null)
            Text(
              l10n.dbmsStatsResetAt(
                DateFormat.yMd().add_Hm().format(statsReset.toLocal()),
              ),
            ),
        ],
      ),
    );
  }
}

class _DeadlockHistorySection extends StatelessWidget {
  final DeadlockInfo info;
  const _DeadlockHistorySection({required this.info});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final statsReset = info.statsReset;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(l10n.dbmsDeadlockHistorySummary(info.deadlocks)),
          if (statsReset != null)
            Text(
              l10n.dbmsStatsResetAt(
                DateFormat.yMd().add_Hm().format(statsReset.toLocal()),
              ),
            ),
        ],
      ),
    );
  }
}

class _LongTransactionsList extends StatelessWidget {
  final List<LongTransaction> transactions;
  const _LongTransactionsList({required this.transactions});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (transactions.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: transactions.length,
      itemBuilder: (context, index) =>
          _LongTransactionRow(transaction: transactions[index]),
    );
  }
}

class _LongTransactionRow extends StatelessWidget {
  final LongTransaction transaction;
  const _LongTransactionRow({required this.transaction});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text('${l10n.dbmsColumnPid}: ${transaction.pid}'),
      subtitle: Text(
        [
          l10n.dbmsLongTransactionRowSubtitle(
            transaction.username ?? '?',
            transaction.state.wireValue,
            transaction.durationSeconds.toStringAsFixed(1),
          ),
          if (transaction.query != null) transaction.query!,
        ].join('  •  '),
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}

class _IdleInTransactionSessionsList extends StatelessWidget {
  final List<IdleInTransactionSession> sessions;
  const _IdleInTransactionSessionsList({required this.sessions});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (sessions.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: sessions.length,
      itemBuilder: (context, index) =>
          _IdleInTransactionSessionRow(session: sessions[index]),
    );
  }
}

class _IdleInTransactionSessionRow extends StatelessWidget {
  final IdleInTransactionSession session;
  const _IdleInTransactionSessionRow({required this.session});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text('${l10n.dbmsColumnPid}: ${session.pid}'),
      subtitle: Text(
        l10n.dbmsIdleInTransactionSessionRowSubtitle(
          session.username ?? '?',
          session.idleDurationSeconds.toStringAsFixed(1),
        ),
      ),
    );
  }
}

String _failureMessage(ApiFailure failure, AppLocalizations l10n) =>
    switch (failure) {
      ApiFailureTimeout() => l10n.connectionTimeout,
      ApiFailureUnavailable() => l10n.connectionUnavailableBody,
      ApiFailureMalformedPayload() => l10n.connectionMalformedPayload,
      ApiFailureServerError(:final error) =>
        error.toJson()['message'] as String? ?? l10n.connectionUnavailableBody,
    };

enum _ExplainStage { idle, loading, loaded, error }

/// Unit U47: `core::ai`'s DB-side explanation (`/analysis/db/explain`,
/// U46/ADR 0051), mirroring `host_analysis_screen.dart`'s own
/// `_ExplanationSection` (ADR 0050) exactly — deliberately on-demand
/// (a button tap fires the request exactly once), never a background-
/// polled `StreamProvider`, since a real AI inference round-trip is
/// genuinely expensive.
class _ExplanationSection extends ConsumerStatefulWidget {
  const _ExplanationSection();

  @override
  ConsumerState<_ExplanationSection> createState() =>
      _ExplanationSectionState();
}

class _ExplanationSectionState extends ConsumerState<_ExplanationSection> {
  _ExplainStage _stage = _ExplainStage.idle;
  GatedValueForAiExplanation? _gated;
  String? _error;

  Future<void> _explain() async {
    final l10n = AppLocalizations.of(context)!;
    setState(() {
      _stage = _ExplainStage.loading;
      _error = null;
    });
    final client = ref.read(apiClientProvider);
    final result = await client.getDbExplanation();
    if (!mounted) return;

    switch (result) {
      case ApiOk(:final value):
        setState(() {
          _gated = value;
          _stage = _ExplainStage.loaded;
        });
      case ApiErr(:final failure):
        setState(() {
          _error = _failureMessage(failure, l10n);
          _stage = _ExplainStage.error;
        });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l10n.analysisExplanationHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        switch (_stage) {
          _ExplainStage.idle => OutlinedButton(
            onPressed: _explain,
            child: Text(l10n.analysisExplainButton),
          ),
          _ExplainStage.loading => const Padding(
            padding: EdgeInsets.symmetric(vertical: 8),
            child: SizedBox(
              width: 24,
              height: 24,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          ),
          _ExplainStage.error => Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Keyed: this section's own transport-failure text uses the
              // same generic `AsyncResultView`-style wording every other
              // (independently-polled) DBMS zone on this screen already
              // shows for its own unrelated real failure — a `Key` lets
              // tests target this section's own message precisely, not
              // an accidental collision with a sibling zone's real text.
              Text(
                _error ?? '',
                key: const Key('dbExplanationError'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
              const SizedBox(height: 8),
              OutlinedButton(
                onPressed: _explain,
                child: Text(l10n.analysisExplainButton),
              ),
            ],
          ),
          _ExplainStage.loaded => _GatedExplanation(
            gated: _gated!,
            onRetry: _explain,
          ),
        },
        const SizedBox(height: 16),
      ],
    );
  }
}

class _GatedExplanation extends StatelessWidget {
  final GatedValueForAiExplanation gated;
  final VoidCallback onRetry;
  const _GatedExplanation({required this.gated, required this.onRetry});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return switch (gated) {
      GatedValueForAiExplanationSupported(:final value) =>
        _ExplanationContent(explanation: value, onRefresh: onRetry),
      GatedValueForAiExplanationLimited(:final reason) => _ExplainGatedMessage(
        icon: Icons.info_outline,
        title: l10n.metricStateLimitedTitle,
        reason: reason,
        onRetry: onRetry,
      ),
      GatedValueForAiExplanationUnavailable(:final reason) => _ExplainGatedMessage(
        icon: Icons.remove_circle_outline,
        title: l10n.metricStateUnavailableTitle,
        reason: reason,
        onRetry: onRetry,
      ),
      GatedValueForAiExplanationPermissionRequired(:final reason) =>
        _ExplainGatedMessage(
          icon: Icons.lock_outline,
          title: l10n.metricStatePermissionRequiredTitle,
          reason: reason,
          onRetry: onRetry,
        ),
    };
  }
}

class _ExplainGatedMessage extends StatelessWidget {
  final IconData icon;
  final String title;
  final String reason;
  final VoidCallback onRetry;
  const _ExplainGatedMessage({
    required this.icon,
    required this.title,
    required this.reason,
    required this.onRetry,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final muted = Theme.of(context).colorScheme.onSurfaceVariant;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(icon, color: muted),
            const SizedBox(width: 8),
            Text(title, style: TextStyle(color: muted)),
          ],
        ),
        const SizedBox(height: 4),
        Text(reason, style: TextStyle(color: muted)),
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: onRetry,
          child: Text(l10n.analysisExplainButton),
        ),
      ],
    );
  }
}

class _ExplanationContent extends StatelessWidget {
  final AiExplanation explanation;
  final VoidCallback onRefresh;
  const _ExplanationContent({
    required this.explanation,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final muted = Theme.of(context).colorScheme.onSurfaceVariant;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(explanation.summary),
        const SizedBox(height: 4),
        Text(
          l10n.analysisExplanationRiskAndConfidence(
            explanation.risk.wireValue,
            (explanation.confidence * 100).round(),
          ),
          style: TextStyle(color: muted),
        ),
        for (final observation in explanation.observations)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text('•  ${observation.text}'),
          ),
        for (final recommendation in explanation.recommendations)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text('→  ${recommendation.text}'),
          ),
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: onRefresh,
          child: Text(l10n.analysisExplainButton),
        ),
      ],
    );
  }
}
