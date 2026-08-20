import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/correlation_result.dart';
import '../generated/models/evidence.dart';
import '../generated/models/hypothesis.dart';
import '../generated/models/root_cause.dart';
import '../generated/models/ruled_out.dart';
import '../l10n/app_localizations.dart';
import '../providers/correlation_providers.dart';
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

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Padding(
      padding: const EdgeInsets.all(16),
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
          for (final entry in ruledOut) _RuledOutRow(ruledOut: entry, l10n: l10n),
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
    RootCause.clientSideApplication => l10n.correlationCauseClientSideApplication,
  };
}
