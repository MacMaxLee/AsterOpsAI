import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/gated_value_for_array_of_query_stat.dart';
import '../generated/models/lock_edge.dart';
import '../generated/models/query_stat.dart';
import '../generated/models/session_info.dart';
import '../l10n/app_localizations.dart';
import '../providers/dbms_providers.dart';
import '../widgets/async_result_view.dart';

/// Unit U32/U34: the console surface for unit U31/U33's own direct DBMS
/// endpoints. Independent sections (`Sessions`, `Locks`, `Query
/// stats`), each watching its own provider, so a slow/failed poll on
/// one never blocks the others — the same shape `DashboardScreen`'s own
/// multiple independently-polled `AsyncResultView` sections already
/// use. Every endpoint here is read-only; there is no mutation UI.
class DatabaseScreen extends ConsumerWidget {
  const DatabaseScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final sessions = ref.watch(dbmsSessionsProvider);
    final locks = ref.watch(dbmsLocksProvider);
    final queryStats = ref.watch(dbmsQueryStatsProvider);

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
