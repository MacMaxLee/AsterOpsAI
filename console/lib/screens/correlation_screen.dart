import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../generated/models/ai_explanation.dart';
import '../generated/models/correlation_result.dart';
import '../generated/models/evidence.dart';
import '../generated/models/gated_value_for_ai_explanation.dart';
import '../generated/models/hypothesis.dart';
import '../generated/models/root_cause.dart';
import '../generated/models/ruled_out.dart';
import '../l10n/app_localizations.dart';
import '../providers/correlation_providers.dart';
import '../providers/transport_provider.dart';
import '../widgets/async_result_view.dart';

/// Unit U20: the cross-layer correlation verdict (U5/U12's
/// `correlate()`, wired for the first time — ADR 0017 forward-referenced
/// exactly this gap). Renders only what `CorrelationResult` carries:
/// ranked hypotheses (cause, confidence, evidence) and the ruled-out list
/// (cause, reason). No client-side re-derivation of any of it
/// (FR-CONSOLE-001) — a cause's presence in `ruledOut` is rendered
/// exactly as honestly as a ranked one, never hidden.
///
/// Unit U21 reworked the layout: root cause, evidence, ruled-out list,
/// and confidence all have to be visible without scrolling or a toggle
/// (the demo's own REQUIREMENTS #5) — a single unbounded `ListView`
/// (U20's first version) doesn't guarantee that once more than one cause
/// is ranked with its own evidence lines. This is a fixed, two-region
/// `Row` instead: no `ListView`/`SingleChildScrollView` anywhere in this
/// screen, deliberately — if a result genuinely doesn't fit, that's a
/// real problem to see (an overflow), not one to quietly paper over with
/// a scrollbar that defeats the requirement's own point.
///
/// Unit U70 adds AI explanation (`build_correlation_bundle`'s own wire
/// surface) behind a button that opens a dialog, rather than inline
/// like `host_analysis_screen.dart`'s `_ExplanationSection` — an
/// unbounded-height inline section would reopen exactly the overflow
/// risk U21's own rework eliminated for the ranked/ruled-out content.
/// The dialog scrolls internally if the explanation is long; the
/// underlying fixed layout U21 requires is untouched.
class CorrelationScreen extends ConsumerWidget {
  const CorrelationScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(correlationProvider);
    return AsyncResultView<CorrelationResult>(
      asyncValue: async,
      builder: (context, result) => _CorrelationBody(result: result),
    );
  }
}

class _CorrelationBody extends StatelessWidget {
  final CorrelationResult result;
  const _CorrelationBody({required this.result});

  void _showExplanationDialog(BuildContext context) {
    showDialog<void>(
      context: context,
      builder: (context) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 480, maxHeight: 480),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: SingleChildScrollView(child: _ExplanationSection()),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Align(
            alignment: Alignment.centerRight,
            child: OutlinedButton(
              key: const Key('correlationExplainTrigger'),
              onPressed: () => _showExplanationDialog(context),
              child: Text(l10n.analysisExplainButton),
            ),
          ),
          const SizedBox(height: 8),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 3,
                  child: _RankedColumn(ranked: result.ranked, l10n: l10n),
                ),
                const SizedBox(width: 16),
                Expanded(
                  flex: 2,
                  child: _RuledOutColumn(ruledOut: result.ruledOut, l10n: l10n),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _RankedColumn extends StatelessWidget {
  final List<Hypothesis> ranked;
  final AppLocalizations l10n;
  const _RankedColumn({required this.ranked, required this.l10n});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          l10n.correlationRankedHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 4),
        if (ranked.isEmpty)
          Text(l10n.genericEmpty)
        else
          for (final hypothesis in ranked)
            _HypothesisBlock(hypothesis: hypothesis, l10n: l10n),
      ],
    );
  }
}

class _HypothesisBlock extends StatelessWidget {
  final Hypothesis hypothesis;
  final AppLocalizations l10n;
  const _HypothesisBlock({required this.hypothesis, required this.l10n});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: 8, bottom: 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  _causeLabel(hypothesis.cause, l10n),
                  style: theme.textTheme.titleSmall,
                ),
              ),
              Text(
                l10n.correlationConfidence(
                  (hypothesis.confidence * 100).toStringAsFixed(0),
                ),
                style: theme.textTheme.bodySmall,
              ),
            ],
          ),
          for (final evidence in hypothesis.evidence)
            Text(
              l10n.correlationEvidenceDetail(
                evidence.metric,
                _formatEvidenceValue(evidence, evidence.observed),
                _formatEvidenceValue(evidence, evidence.threshold),
              ),
              style: theme.textTheme.bodySmall,
            ),
          const Divider(height: 8),
        ],
      ),
    );
  }

  String _formatEvidenceValue(Evidence evidence, double value) {
    final unit = evidence.unit;
    final formatted = value.toStringAsFixed(2);
    return unit == null ? formatted : '$formatted $unit';
  }
}

class _RuledOutColumn extends StatelessWidget {
  final List<RuledOut> ruledOut;
  final AppLocalizations l10n;
  const _RuledOutColumn({required this.ruledOut, required this.l10n});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          l10n.correlationRuledOutHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 4),
        if (ruledOut.isEmpty)
          Text(l10n.genericEmpty)
        else
          for (final entry in ruledOut)
            _RuledOutRow(ruledOut: entry, l10n: l10n),
      ],
    );
  }
}

class _RuledOutRow extends StatelessWidget {
  final RuledOut ruledOut;
  final AppLocalizations l10n;
  const _RuledOutRow({required this.ruledOut, required this.l10n});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            _causeLabel(ruledOut.cause, l10n),
            style: theme.textTheme.bodyMedium,
          ),
          Text(
            ruledOut.reason,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

String _causeLabel(RootCause cause, AppLocalizations l10n) {
  return switch (cause) {
    RootCause.dbLocks => l10n.correlationCauseDbLocks,
    RootCause.dbConfiguration => l10n.correlationCauseDbConfiguration,
    RootCause.connectionExhaustion => l10n.correlationCauseConnectionExhaustion,
    RootCause.slowSql => l10n.correlationCauseSlowSql,
    RootCause.hostCpu => l10n.correlationCauseHostCpu,
    RootCause.hostMemory => l10n.correlationCauseHostMemory,
    RootCause.storageLatency => l10n.correlationCauseStorageLatency,
    RootCause.network => l10n.correlationCauseNetwork,
    RootCause.clientSideApplication =>
      l10n.correlationCauseClientSideApplication,
  };
}

String _failureMessage(ApiFailure failure, AppLocalizations l10n) =>
    switch (failure) {
      ApiFailureTimeout() => l10n.connectionTimeout,
      ApiFailureUnavailable() => l10n.connectionUnavailableBody,
      ApiFailureMalformedPayload() => l10n.connectionMalformedPayload,
      ApiFailureServerError(:final error) =>
        error.toJson()['message'] as String? ?? l10n.connectionUnavailableBody,
    };

enum _ExplainStage { loading, loaded, error }

/// Unit U70: mirrors `host_analysis_screen.dart`'s own
/// `_ExplanationSection` (ADR 0050) exactly, except it fires
/// immediately on open rather than waiting for an `idle`-state button
/// tap — the user already tapped "Explain with AI" once, to open the
/// dialog this lives in; a second tap inside it would be redundant.
class _ExplanationSection extends ConsumerStatefulWidget {
  const _ExplanationSection();

  @override
  ConsumerState<_ExplanationSection> createState() =>
      _ExplanationSectionState();
}

class _ExplanationSectionState extends ConsumerState<_ExplanationSection> {
  _ExplainStage _stage = _ExplainStage.loading;
  GatedValueForAiExplanation? _gated;
  String? _error;
  bool _started = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Not `initState`: `AppLocalizations.of(context)` (inside `_explain`)
    // depends on an inherited widget, which isn't safe to look up until
    // dependencies are resolved — `didChangeDependencies` is the real
    // "run once, as soon as `context` is usable" hook, guarded so a
    // later dependency change (e.g. a locale change) doesn't refire it.
    if (!_started) {
      _started = true;
      _explain();
    }
  }

  Future<void> _explain() async {
    final l10n = AppLocalizations.of(context)!;
    setState(() {
      _stage = _ExplainStage.loading;
      _error = null;
    });
    final client = ref.read(apiClientProvider);
    final result = await client.getCorrelationExplanation();
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
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          l10n.analysisExplanationHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        switch (_stage) {
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
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                _error ?? '',
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
      GatedValueForAiExplanationSupported(:final value) => _ExplanationContent(
        explanation: value,
        onRefresh: onRetry,
      ),
      GatedValueForAiExplanationLimited(:final reason) => _GatedMessage(
        icon: Icons.info_outline,
        title: l10n.metricStateLimitedTitle,
        reason: reason,
        onRetry: onRetry,
      ),
      GatedValueForAiExplanationUnavailable(:final reason) => _GatedMessage(
        icon: Icons.remove_circle_outline,
        title: l10n.metricStateUnavailableTitle,
        reason: reason,
        onRetry: onRetry,
      ),
      GatedValueForAiExplanationPermissionRequired(:final reason) =>
        _GatedMessage(
          icon: Icons.lock_outline,
          title: l10n.metricStatePermissionRequiredTitle,
          reason: reason,
          onRetry: onRetry,
        ),
    };
  }
}

class _GatedMessage extends StatelessWidget {
  final IconData icon;
  final String title;
  final String reason;
  final VoidCallback onRetry;
  const _GatedMessage({
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
      mainAxisSize: MainAxisSize.min,
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
      mainAxisSize: MainAxisSize.min,
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
