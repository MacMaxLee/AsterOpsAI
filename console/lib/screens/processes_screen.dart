import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../generated/models/action_proposal_outcome.dart';
import '../generated/models/process_info.dart';
import '../generated/models/process_snapshot.dart';
import '../generated/models/resumable_action_summary.dart';
import '../generated/models/tuning_candidate_outcome.dart';
import '../generated/models/tuning_plan_outcome.dart';
import '../l10n/app_localizations.dart';
import '../providers/telemetry_providers.dart';
import '../providers/transport_provider.dart';
import '../widgets/async_result_view.dart';
import '../widgets/formatters.dart';
import '../widgets/metric_display.dart';

class ProcessesScreen extends ConsumerWidget {
  const ProcessesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(processesProvider);
    return AsyncResultView<ProcessSnapshot>(
      asyncValue: async,
      builder: (context, snapshot) => _ProcessesBody(snapshot: snapshot),
    );
  }
}

class _ProcessesBody extends StatelessWidget {
  final ProcessSnapshot snapshot;
  const _ProcessesBody({required this.snapshot});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (snapshot.processes.isEmpty) {
      return Center(child: Text(l10n.genericEmpty));
    }
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(8),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(l10n.processesTotalCount(snapshot.totalCount)),
          ),
        ),
        Expanded(
          child: ListView.builder(
            itemCount: snapshot.processes.length,
            itemBuilder: (context, index) =>
                _ProcessRow(process: snapshot.processes[index]),
          ),
        ),
      ],
    );
  }
}

class _ProcessRow extends StatelessWidget {
  final ProcessInfo process;
  const _ProcessRow({required this.process});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      title: Text(process.comm),
      subtitle: Text(
        '${l10n.processColumnPid}: ${process.pid}  •  ${l10n.processColumnCategory}: ${process.category.wireValue}',
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          MetricValueText.double(
            process.cpuPercent,
            (v) => '${v.toStringAsFixed(1)}%',
          ),
          const SizedBox(width: 16),
          MetricValueText.uint64(process.rssBytes, formatBytes),
          IconButton(
            icon: const Icon(Icons.tune_outlined),
            tooltip: l10n.processStartTuningPlan,
            onPressed: () => showDialog<void>(
              context: context,
              builder: (_) => _StartTuningPlanDialog(process: process),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.pause_circle_outlined),
            tooltip: l10n.processSuspendProcess,
            onPressed: () => showDialog<void>(
              context: context,
              builder: (_) => _SuspendProcessDialog(process: process),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.play_circle_outlined),
            tooltip: l10n.processResumeProcess,
            onPressed: () => showDialog<void>(
              context: context,
              builder: (_) => _ResumeProcessDialog(process: process),
            ),
          ),
        ],
      ),
    );
  }
}

const _tuningProfiles = [
  'BALANCED',
  'HIGH_PERFORMANCE',
  'BATTERY_SAVER',
  'DEVELOPMENT',
];
const _automationModes = [
  'RECOMMEND_ONLY',
  'ASK_BEFORE_CHANGES',
  'AUTO_LOW_RISK',
];

/// Unit U24: `pid`/`start_time_ticks` come straight from the real,
/// already-fetched `ProcessInfo` row this dialog was opened from — never
/// typed by hand, since an operator has no way to know a process's real
/// `start_time_ticks` (`/proc/[pid]/stat` field 22) themselves.
class _StartTuningPlanDialog extends ConsumerStatefulWidget {
  final ProcessInfo process;
  const _StartTuningPlanDialog({required this.process});

  @override
  ConsumerState<_StartTuningPlanDialog> createState() =>
      _StartTuningPlanDialogState();
}

class _StartTuningPlanDialogState
    extends ConsumerState<_StartTuningPlanDialog> {
  final _formKey = GlobalKey<FormState>();
  late final _resourceNameController = TextEditingController(
    text: widget.process.comm,
  );
  String _profile = _tuningProfiles.first;
  String _mode = _automationModes.first;
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _resourceNameController.dispose();
    super.dispose();
  }

  // No auth/session system exists yet (ADR 0018/0021/0027's own flagged
  // limitation) — same plain operator name every console mutation uses.
  static const _operator = 'console-operator';

  Future<void> _submit() async {
    final l10n = AppLocalizations.of(context)!;
    if (!(_formKey.currentState?.validate() ?? false)) return;

    setState(() {
      _error = null;
      _submitting = true;
    });

    final client = ref.read(apiClientProvider);
    final result = await client.startTuningPlan(
      pid: widget.process.pid,
      startTimeTicks: widget.process.startTimeTicks,
      resourceName: _resourceNameController.text.trim(),
      profile: _profile,
      mode: _mode,
      requestedBy: _operator,
    );
    if (!mounted) return;

    switch (result) {
      case ApiOk(:final value):
        Navigator.of(context).pop();
        showDialog<void>(
          context: context,
          builder: (_) => _TuningPlanResultDialog(outcome: value),
        );
      case ApiErr(:final failure):
        setState(() {
          _submitting = false;
          _error = _failureMessage(failure, l10n);
        });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return AlertDialog(
      title: Text(l10n.tuningStartDialogTitle),
      content: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextFormField(
              controller: _resourceNameController,
              decoration: InputDecoration(
                labelText: l10n.tuningStartResourceName,
              ),
              validator: (v) => (v == null || v.trim().isEmpty) ? '' : null,
            ),
            DropdownButtonFormField<String>(
              value: _profile,
              decoration: InputDecoration(labelText: l10n.tuningStartProfile),
              items: [
                for (final profile in _tuningProfiles)
                  DropdownMenuItem(value: profile, child: Text(profile)),
              ],
              onChanged: (v) => setState(() => _profile = v ?? _profile),
            ),
            DropdownButtonFormField<String>(
              value: _mode,
              decoration: InputDecoration(labelText: l10n.tuningStartMode),
              items: [
                for (final mode in _automationModes)
                  DropdownMenuItem(value: mode, child: Text(mode)),
              ],
              onChanged: (v) => setState(() => _mode = v ?? _mode),
            ),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  _error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.genericCancel),
        ),
        FilledButton(
          onPressed: _submitting ? null : _submit,
          child: _submitting
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.tuningStartSubmit),
        ),
      ],
    );
  }
}

/// Renders every candidate's real `action_type`/`outcome` verbatim
/// (FR-CONSOLE-001) — never recolored, iconified, or reclassified. Per
/// ADR 0028, the real outcome is often not what a caller might assume
/// (e.g. `AUTO_ALLOWED_PENDING` rather than `PENDING_APPROVAL`), so this
/// dialog is the one place that reality is shown honestly rather than
/// silently swallowed the way a plain success snackbar would.
class _TuningPlanResultDialog extends StatelessWidget {
  final TuningPlanOutcome outcome;
  const _TuningPlanResultDialog({required this.outcome});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return AlertDialog(
      title: Text(l10n.tuningStartResultTitle),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(outcome.status),
          const SizedBox(height: 8),
          for (final candidate in outcome.candidates)
            _CandidateOutcomeRow(candidate: candidate),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.genericClose),
        ),
      ],
    );
  }
}

class _CandidateOutcomeRow extends StatelessWidget {
  final TuningCandidateOutcome candidate;
  const _CandidateOutcomeRow({required this.candidate});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Text(
        l10n.tuningStartResultCandidate(
          candidate.actionType,
          candidate.outcome,
        ),
      ),
    );
  }
}

/// Unit U27: `security.suspend_process` (real `SIGSTOP`, wired in unit
/// U26) takes no parameters — unlike `_StartTuningPlanDialog`, there is
/// nothing to configure, so this is a plain confirmation, not a `Form`.
/// A genuinely disruptive action (it freezes a real process, even
/// though only once approved) deserves a real confirm step, not a
/// bare one-tap button.
class _SuspendProcessDialog extends ConsumerStatefulWidget {
  final ProcessInfo process;
  const _SuspendProcessDialog({required this.process});

  @override
  ConsumerState<_SuspendProcessDialog> createState() =>
      _SuspendProcessDialogState();
}

class _SuspendProcessDialogState extends ConsumerState<_SuspendProcessDialog> {
  bool _submitting = false;
  String? _error;

  // No auth/session system exists yet (ADR 0018/0021/0027's own flagged
  // limitation) — same plain operator name every console mutation uses.
  static const _operator = 'console-operator';

  Future<void> _submit() async {
    final l10n = AppLocalizations.of(context)!;
    setState(() {
      _error = null;
      _submitting = true;
    });

    final client = ref.read(apiClientProvider);
    final result = await client.proposeAction(
      actionType: 'security.suspend_process',
      pid: widget.process.pid,
      startTimeTicks: widget.process.startTimeTicks,
      resourceName: widget.process.comm,
      requestedBy: _operator,
    );
    if (!mounted) return;

    switch (result) {
      case ApiOk(:final value):
        Navigator.of(context).pop();
        showDialog<void>(
          context: context,
          builder: (_) => _ActionProposalResultDialog(outcome: value),
        );
      case ApiErr(:final failure):
        setState(() {
          _submitting = false;
          _error = _failureMessage(failure, l10n);
        });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return AlertDialog(
      title: Text(l10n.suspendProcessConfirmTitle),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.suspendProcessConfirmBody(
              widget.process.comm,
              widget.process.pid,
            ),
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.genericCancel),
        ),
        FilledButton(
          onPressed: _submitting ? null : _submit,
          child: _submitting
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.suspendProcessConfirmSubmit),
        ),
      ],
    );
  }
}

/// Renders the real `status`/`row_id` verbatim (FR-CONSOLE-001) — no
/// narrative added about what happens next; the status string itself
/// already says `PENDING_APPROVAL`, and the existing Policy inbox
/// screen (unit U16) is where an operator would look next regardless.
class _ActionProposalResultDialog extends StatelessWidget {
  final ActionProposalOutcome outcome;
  const _ActionProposalResultDialog({required this.outcome});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return AlertDialog(
      title: Text(l10n.actionProposalResultTitle),
      content: Text(
        l10n.actionProposalResultStatus(outcome.status, outcome.rowId),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.genericClose),
        ),
      ],
    );
  }
}

/// Unit U29: the discovery lookup (`GET /actions/resumable`) fires once,
/// on this dialog's own first frame — never a background poll on every
/// `ProcessesScreen` row, which would be a real, avoidable cost since
/// the screen already polls `/processes` continuously. Not a `Form`:
/// there's nothing to configure, same reasoning as `_SuspendProcessDialog`.
class _ResumeProcessDialog extends ConsumerStatefulWidget {
  final ProcessInfo process;
  const _ResumeProcessDialog({required this.process});

  @override
  ConsumerState<_ResumeProcessDialog> createState() =>
      _ResumeProcessDialogState();
}

enum _ResumeStage { checking, found, notFound, submitting }

class _ResumeProcessDialogState extends ConsumerState<_ResumeProcessDialog> {
  _ResumeStage _stage = _ResumeStage.checking;
  ResumableActionSummary? _resumable;
  String? _error;
  bool _checkStarted = false;

  // No auth/session system exists yet (ADR 0018/0021/0027's own flagged
  // limitation) — same plain operator name every console mutation uses.
  static const _operator = 'console-operator';

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Not `initState`: `AppLocalizations.of(context)` depends on an
    // `InheritedWidget`, which isn't available yet at `initState` time.
    // `didChangeDependencies` can fire more than once, so `_checkStarted`
    // guards this to a genuine one-shot lookup on the dialog's first
    // frame — never a repeated or background poll.
    if (!_checkStarted) {
      _checkStarted = true;
      _check();
    }
  }

  Future<void> _check() async {
    final l10n = AppLocalizations.of(context)!;
    final client = ref.read(apiClientProvider);
    final result = await client.getResumableActions(
      pid: widget.process.pid,
      startTimeTicks: widget.process.startTimeTicks,
    );
    if (!mounted) return;

    switch (result) {
      case ApiOk(:final value):
        setState(() {
          _resumable = value.isEmpty ? null : value.first;
          _stage = value.isEmpty ? _ResumeStage.notFound : _ResumeStage.found;
        });
      case ApiErr(:final failure):
        setState(() {
          _error = _failureMessage(failure, l10n);
          _stage = _ResumeStage.notFound;
        });
    }
  }

  Future<void> _submit() async {
    final l10n = AppLocalizations.of(context)!;
    final resumable = _resumable;
    if (resumable == null) return;

    setState(() {
      _error = null;
      _stage = _ResumeStage.submitting;
    });

    final client = ref.read(apiClientProvider);
    final result = await client.rollbackAction(resumable.rowId, _operator);
    if (!mounted) return;

    switch (result) {
      case ApiOk():
        Navigator.of(context).pop();
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              l10n.resumeProcessSuccessSnackbar(
                resumable.actionType,
                resumable.rowId,
              ),
            ),
          ),
        );
      case ApiErr(:final failure):
        setState(() {
          _error = _failureMessage(failure, l10n);
          _stage = _ResumeStage.found;
        });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final dateFormat = DateFormat.yMd().add_Hm();
    final resumable = _resumable;

    return AlertDialog(
      title: Text(l10n.resumeProcessDialogTitle),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (_stage == _ResumeStage.checking)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Center(
                child: SizedBox(
                  width: 24,
                  height: 24,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              ),
            )
          else if (resumable != null)
            Text(
              l10n.resumeProcessFound(
                resumable.actionType,
                dateFormat.format(resumable.executedAt.toLocal()),
              ),
            )
          else
            Text(l10n.resumeProcessNotFound),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: _stage == _ResumeStage.submitting
              ? null
              : () => Navigator.of(context).pop(),
          child: Text(
            resumable == null ? l10n.genericClose : l10n.genericCancel,
          ),
        ),
        if (resumable != null)
          FilledButton(
            onPressed: _stage == _ResumeStage.submitting ? null : _submit,
            child: _stage == _ResumeStage.submitting
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : Text(l10n.resumeProcessSubmit),
          ),
      ],
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
