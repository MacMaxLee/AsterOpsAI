import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../generated/models/tuning_plan_summary.dart';
import '../l10n/app_localizations.dart';
import '../providers/tuning_providers.dart';
import '../widgets/async_result_view.dart';

/// Unit U17: tuning plan history, read-only (U14's own endpoint is
/// read-only too — no mutation to wire). Deliberately shows neither
/// `targetIdentityJson` nor `candidatesJson` — same reasoning ADR 0019
/// already gave on the Rust side (no clean projection exists for either),
/// reinforced here by FR-CONSOLE-001: parsing either client-side would be
/// exactly the kind of business logic that requirement forbids.
class TuningHistoryScreen extends ConsumerWidget {
  const TuningHistoryScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(tuningPlansProvider);
    return AsyncResultView<List<TuningPlanSummary>>(
      asyncValue: async,
      builder: (context, plans) => _TuningHistoryBody(plans: plans),
    );
  }
}

class _TuningHistoryBody extends StatelessWidget {
  final List<TuningPlanSummary> plans;
  const _TuningHistoryBody({required this.plans});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (plans.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return ListView.builder(
      itemCount: plans.length,
      itemBuilder: (context, index) => _TuningPlanRow(plan: plans[index]),
    );
  }
}

class _TuningPlanRow extends StatelessWidget {
  final TuningPlanSummary plan;
  const _TuningPlanRow({required this.plan});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final dateFormat = DateFormat.yMd().add_Hm();
    final completedAt = plan.completedAt;
    final subtitle = [
      plan.status,
      dateFormat.format(plan.createdAt.toLocal()),
      if (completedAt != null)
        l10n.tuningCompletedAt(dateFormat.format(completedAt.toLocal())),
    ].join('  •  ');

    return ListTile(
      title: Text('${plan.profile} · ${plan.mode}'),
      subtitle: Text(subtitle),
    );
  }
}
