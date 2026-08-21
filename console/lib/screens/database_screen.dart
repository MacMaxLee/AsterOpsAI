import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/lock_edge.dart';
import '../generated/models/session_info.dart';
import '../l10n/app_localizations.dart';
import '../providers/dbms_providers.dart';
import '../widgets/async_result_view.dart';

/// Unit U32: the first console surface for unit U31's own direct DBMS
/// endpoints. Two independent sections (`Sessions`, `Locks`), each
/// watching its own provider, so a slow/failed poll on one never blocks
/// the other — the same shape `DashboardScreen`'s own multiple
/// independently-polled `AsyncResultView` sections already use. Both
/// endpoints are read-only; there is no mutation UI here.
class DatabaseScreen extends ConsumerWidget {
  const DatabaseScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final sessions = ref.watch(dbmsSessionsProvider);
    final locks = ref.watch(dbmsLocksProvider);

    return Row(
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
