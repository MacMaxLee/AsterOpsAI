import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../generated/models/domain_signal.dart';
import '../generated/models/host_bottleneck.dart';
import '../generated/models/host_domain.dart';
import '../generated/models/host_verdict.dart';
import '../generated/models/tier.dart';
import '../l10n/app_localizations.dart';
import '../providers/analysis_providers.dart';
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
