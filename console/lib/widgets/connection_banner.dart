import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../providers/connection_status.dart';

/// Sits above the active screen's content. Invisible when
/// `ConnectionConnected` — the banner only ever appears to explain *why*
/// data isn't fresh, never to restate that it is.
class ConnectionBanner extends ConsumerWidget {
  const ConnectionBanner({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(connectionStatusProvider);
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    final (String message, IconData icon, bool showRetry) = switch (status) {
      ConnectionConnecting() => (l10n.connectionConnecting, Icons.sync, false),
      ConnectionConnected() => ('', Icons.check_circle, false),
      ConnectionReconnecting() => (
        l10n.connectionReconnecting,
        Icons.sync_problem,
        false,
      ),
      ConnectionUnavailable() => (
        l10n.connectionUnavailableBody,
        Icons.cloud_off,
        true,
      ),
      ConnectionTimeout() => (l10n.connectionTimeout, Icons.timer_off, true),
      ConnectionMalformedPayload() => (
        l10n.connectionMalformedPayload,
        Icons.error_outline,
        true,
      ),
      ConnectionVersionMismatch() => ('', Icons.system_update, false),
    };

    if (status is ConnectionConnected || status is ConnectionVersionMismatch) {
      return const SizedBox.shrink();
    }

    return Material(
      color: theme.colorScheme.surfaceContainerHighest,
      child: Semantics(
        liveRegion: true,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Row(
            children: [
              Icon(icon, size: 18, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  message,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              if (showRetry)
                TextButton(
                  onPressed: () =>
                      ref.read(connectionStatusProvider.notifier).retryNow(),
                  child: Text(l10n.connectionRetryNow),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
