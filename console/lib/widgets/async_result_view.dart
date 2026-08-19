import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../l10n/app_localizations.dart';

/// Shared "loading / per-call failure / data" handling so every screen
/// doesn't re-decide how to render each `ApiFailure` variant. The
/// connection-level banner (widgets/connection_banner.dart) already covers
/// sustained outages; this covers a single poll's result.
class AsyncResultView<T> extends StatelessWidget {
  final AsyncValue<ApiResult<T>> asyncValue;
  final Widget Function(BuildContext context, T data) builder;

  const AsyncResultView({
    required this.asyncValue,
    required this.builder,
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return asyncValue.when(
      loading: () => Center(
        child: Semantics(
          liveRegion: true,
          label: l10n.genericLoading,
          child: const CircularProgressIndicator(),
        ),
      ),
      error: (error, stackTrace) =>
          _FailureBody(message: l10n.connectionUnavailableBody),
      data: (result) => switch (result) {
        ApiOk(:final value) => builder(context, value),
        ApiErr(:final failure) => _failureFor(failure, l10n),
      },
    );
  }

  Widget _failureFor(ApiFailure failure, AppLocalizations l10n) {
    final message = switch (failure) {
      ApiFailureTimeout() => l10n.connectionTimeout,
      ApiFailureUnavailable() => l10n.connectionUnavailableBody,
      ApiFailureMalformedPayload() => l10n.connectionMalformedPayload,
      ApiFailureServerError(:final error) =>
        error.toJson()['message'] as String? ?? l10n.connectionUnavailableBody,
    };
    return _FailureBody(message: message);
  }
}

class _FailureBody extends StatelessWidget {
  final String message;
  const _FailureBody({required this.message});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Semantics(
          liveRegion: true,
          child: Text(
            message,
            textAlign: TextAlign.center,
            style: TextStyle(color: theme.colorScheme.onSurfaceVariant),
          ),
        ),
      ),
    );
  }
}
