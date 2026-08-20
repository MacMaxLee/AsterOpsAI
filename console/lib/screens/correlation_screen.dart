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
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Text(
          l10n.correlationRankedHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        if (result.ranked.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Text(l10n.genericEmpty),
          )
        else
          for (final hypothesis in result.ranked)
            _HypothesisCard(hypothesis: hypothesis),
        const SizedBox(height: 24),
        Text(
          l10n.correlationRuledOutHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        if (result.ruledOut.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Text(l10n.genericEmpty),
          )
        else
          for (final ruledOut in result.ruledOut) _RuledOutRow(ruledOut: ruledOut),
      ],
    );
  }
}

class _HypothesisCard extends StatelessWidget {
  final Hypothesis hypothesis;
  const _HypothesisCard({required this.hypothesis});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              _causeLabel(hypothesis.cause, l10n),
              style: Theme.of(context).textTheme.titleSmall,
            ),
            Text(
              l10n.correlationConfidence(
                (hypothesis.confidence * 100).toStringAsFixed(0),
              ),
            ),
            for (final evidence in hypothesis.evidence)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  l10n.correlationEvidenceDetail(
                    evidence.metric,
                    _formatEvidenceValue(evidence, evidence.observed),
                    _formatEvidenceValue(evidence, evidence.threshold),
                  ),
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
          ],
        ),
      ),
    );
  }

  String _formatEvidenceValue(Evidence evidence, double value) {
    final unit = evidence.unit;
    final formatted = value.toStringAsFixed(2);
    return unit == null ? formatted : '$formatted $unit';
  }
}

class _RuledOutRow extends StatelessWidget {
  final RuledOut ruledOut;
  const _RuledOutRow({required this.ruledOut});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      dense: true,
      title: Text(_causeLabel(ruledOut.cause, l10n)),
      subtitle: Text(ruledOut.reason),
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
