import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../generated/models/ai_explanation.dart';
import '../generated/models/domain_signal.dart';
import '../generated/models/gated_value_for_ai_explanation.dart';
import '../generated/models/host_bottleneck.dart';
import '../generated/models/host_domain.dart';
import '../generated/models/host_verdict.dart';
import '../generated/models/tier.dart';
import '../l10n/app_localizations.dart';
import '../providers/analysis_providers.dart';
import '../providers/transport_provider.dart';
import '../widgets/async_result_view.dart';

/// Unit U19: host performance analysis (U5's `classify_host`, wired for
/// the first time — ADR 0017 forward-referenced exactly this gap). Renders
/// only what `HostVerdict` carries: bottleneck, score, and a row per
/// domain signal. No client-side re-derivation of any of it
/// (FR-CONSOLE-001) — `HostBottleneck.unknown` (insufficient history) is
/// rendered exactly like any other bottleneck value, never hidden or
/// second-guessed into a fabricated "no bottleneck" reading.
class HostAnalysisScreen extends ConsumerWidget {
  const HostAnalysisScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(hostAnalysisProvider);
    return AsyncResultView<HostVerdict>(
      asyncValue: async,
      builder: (context, verdict) => _HostAnalysisBody(verdict: verdict),
    );
  }
}

class _HostAnalysisBody extends StatelessWidget {
  final HostVerdict verdict;
  const _HostAnalysisBody({required this.verdict});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Card(
          child: ListTile(
            leading: const Icon(Icons.speed_outlined),
            title: Text(_bottleneckLabel(verdict.bottleneck, l10n)),
            subtitle: Text(
              l10n.analysisScore(verdict.score, verdict.scoreVersion),
            ),
          ),
        ),
        const SizedBox(height: 16),
        Text(
          l10n.analysisDomainSignalsHeading,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        if (verdict.domainSignals.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Text(l10n.genericEmpty),
          )
        else
          for (final signal in verdict.domainSignals)
            _DomainSignalRow(signal: signal),
        const Divider(height: 32),
        const _ExplanationSection(),
      ],
    );
  }

  String _bottleneckLabel(HostBottleneck bottleneck, AppLocalizations l10n) {
    return switch (bottleneck) {
      HostBottleneck.none => l10n.analysisBottleneckNone,
      HostBottleneck.cpu => l10n.analysisBottleneckCpu,
      HostBottleneck.memory => l10n.analysisBottleneckMemory,
      HostBottleneck.storageIo => l10n.analysisBottleneckStorageIo,
      HostBottleneck.network => l10n.analysisBottleneckNetwork,
      HostBottleneck.thermal => l10n.analysisBottleneckThermal,
      HostBottleneck.power => l10n.analysisBottleneckPower,
      HostBottleneck.background => l10n.analysisBottleneckBackground,
      HostBottleneck.multiple => l10n.analysisBottleneckMultiple,
      HostBottleneck.unknown => l10n.analysisBottleneckUnknown,
    };
  }
}

class _DomainSignalRow extends StatelessWidget {
  final DomainSignal signal;
  const _DomainSignalRow({required this.signal});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text(_domainLabel(signal.domain, l10n)),
      subtitle: Text(
        l10n.analysisDomainSignalDetail(
          _tierLabel(signal.tier, l10n),
          signal.crossedCount,
          signal.sampleCount,
        ),
      ),
    );
  }

  String _domainLabel(HostDomain domain, AppLocalizations l10n) {
    return switch (domain) {
      HostDomain.cpu => l10n.analysisDomainCpu,
      HostDomain.memory => l10n.analysisDomainMemory,
      HostDomain.storageIo => l10n.analysisDomainStorageIo,
      HostDomain.network => l10n.analysisDomainNetwork,
    };
  }

  String _tierLabel(Tier tier, AppLocalizations l10n) {
    return switch (tier) {
      Tier.normal => l10n.analysisTierNormal,
      Tier.elevated => l10n.analysisTierElevated,
      Tier.high => l10n.analysisTierHigh,
      Tier.critical => l10n.analysisTierCritical,
    };
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

/// Unit U45: deliberately on-demand (a button tap fires the request
/// exactly once), never a background-polled `StreamProvider` — a real
/// AI inference round-trip is genuinely expensive, unlike every other
/// endpoint this console polls on a 1-10s cadence. Mirrors
/// `processes_screen.dart`'s own `_ResumeProcessDialogState` on-demand-
/// fetch shape (a simple stage enum + `ref.read(apiClientProvider)`,
/// not `ref.watch` a stream). See docs/adr/0050.
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
    final result = await client.getHostExplanation();
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
