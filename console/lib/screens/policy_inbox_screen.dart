import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../api/api_client.dart';
import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../generated/models/pending_action_summary.dart';
import '../l10n/app_localizations.dart';
import '../providers/policy_providers.dart';
import '../providers/transport_provider.dart';
import '../widgets/async_result_view.dart';

/// Unit U16: list pending policy actions, grant or reject one. No
/// business logic (FR-CONSOLE-001) — every field rendered here comes
/// straight from `PendingActionSummary`; risk/status text is never
/// recomputed client-side.
class PolicyInboxScreen extends ConsumerWidget {
  const PolicyInboxScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(pendingActionsProvider);
    return AsyncResultView<List<PendingActionSummary>>(
      asyncValue: async,
      builder: (context, actions) => _PolicyInboxBody(actions: actions),
    );
  }
}

class _PolicyInboxBody extends StatelessWidget {
  final List<PendingActionSummary> actions;
  const _PolicyInboxBody({required this.actions});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (actions.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: actions.length,
      itemBuilder: (context, index) =>
          _PendingActionRow(action: actions[index]),
    );
  }
}

class _PendingActionRow extends ConsumerStatefulWidget {
  final PendingActionSummary action;
  const _PendingActionRow({required this.action});

  @override
  ConsumerState<_PendingActionRow> createState() => _PendingActionRowState();
}

enum _InFlight { none, granting, rejecting }

class _PendingActionRowState extends ConsumerState<_PendingActionRow> {
  _InFlight _inFlight = _InFlight.none;

  // No auth/session system exists yet (ADR 0018's own flagged
  // limitation) — the console names its own operator the same plain way
  // core/tests' own approval::grant calls already do.
  static const _operator = 'console-operator';

  Future<void> _grant() => _run(
    _InFlight.granting,
    (client) => client.grantAction(widget.action.id, _operator),
  );

  Future<void> _reject() => _run(
    _InFlight.rejecting,
    (client) => client.rejectAction(widget.action.id, _operator),
  );

  Future<void> _run(
    _InFlight kind,
    Future<ApiResult<void>> Function(ApiClient client) call,
  ) async {
    setState(() => _inFlight = kind);
    final client = ref.read(apiClientProvider);
    final result = await call(client);
    if (!mounted) return;
    setState(() => _inFlight = _InFlight.none);

    switch (result) {
      case ApiOk():
        // A real re-fetch, not an optimistic local removal — the row
        // disappears once the server itself confirms the new status.
        ref.invalidate(pendingActionsProvider);
      case ApiErr(:final failure):
        final l10n = AppLocalizations.of(context)!;
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(_failureMessage(failure, l10n))));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final action = widget.action;
    final dateFormat = DateFormat.yMd().add_Hm();
    final subtitleParts = [
      '${l10n.policyColumnResource}: ${action.resourceKind}/${action.resourceName}',
      '${l10n.policyColumnRequestedBy}: ${action.requestedBy}',
      '${l10n.policyColumnRisk}: ${action.riskClassification}',
      if (action.approvalExpiresAt != null)
        '${l10n.policyColumnExpires}: ${dateFormat.format(action.approvalExpiresAt!.toLocal())}',
    ];

    return ListTile(
      title: Text(action.actionType),
      subtitle: Text(subtitleParts.join('  •  ')),
      trailing: _inFlight != _InFlight.none
          ? const SizedBox(
              width: 24,
              height: 24,
              child: CircularProgressIndicator(strokeWidth: 2),
            )
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextButton(onPressed: _reject, child: Text(l10n.policyReject)),
                const SizedBox(width: 8),
                FilledButton(onPressed: _grant, child: Text(l10n.policyGrant)),
              ],
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
