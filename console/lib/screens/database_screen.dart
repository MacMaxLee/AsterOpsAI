import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/gated_value_for_array_of_query_stat.dart';
import '../generated/models/guc_value.dart';
import '../generated/models/index_stat.dart';
import '../generated/models/lock_edge.dart';
import '../generated/models/query_stat.dart';
import '../generated/models/replication_status.dart';
import '../generated/models/session_info.dart';
import '../generated/models/standby_info.dart';
import '../generated/models/table_stat.dart';
import '../l10n/app_localizations.dart';
import '../providers/dbms_providers.dart';
import '../widgets/async_result_view.dart';

/// Unit U32/U34/U36/U38: the console surface for unit U31/U33/U35/U37's
/// own direct DBMS endpoints. Independent sections (`Sessions`,
/// `Locks`, `Query stats`, `Table stats`, `Index stats`, `Replication`,
/// `Settings`), each watching its own provider, so a slow/failed poll
/// on one never blocks the others — the same shape `DashboardScreen`'s
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

    return Column(
      children: [
        Expanded(
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
        Expanded(
          child: _Section(
            title: l10n.dbmsSectionQueryStats,
            child: AsyncResultView<GatedValueForArrayOfQueryStat>(
              asyncValue: queryStats,
              builder: (context, gated) => _QueryStatsSection(gated: gated),
            ),
          ),
        ),
        const Divider(height: 1),
        Expanded(
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
        Expanded(
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
      ],
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
