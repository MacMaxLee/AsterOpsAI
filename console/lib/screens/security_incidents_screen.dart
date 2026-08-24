import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../api/api_failure.dart';
import '../api/api_result.dart';
import '../generated/models/security_incident_summary.dart';
import '../l10n/app_localizations.dart';
import '../providers/security_providers.dart';
import '../providers/transport_provider.dart';
import '../widgets/async_result_view.dart';

/// Unit U18: open security incidents (read-only), plus a standalone
/// "suppress a detector" action. Deliberately NOT a per-row suppress
/// button — `SecurityIncidentSummary` carries no `detector_id`/resource
/// info to build one from (the summary is an event *count*, not the
/// events themselves); suppression here targets a detector rule
/// directly, the same real, pre-emptive use ADR 0020 already described.
///
/// Unit U76 (docs/adr/0081) adds a real per-row close action —
/// unlike suppress, this needs only the `id` every summary already
/// carries, so no incident-detail endpoint was needed to unblock it.
class SecurityIncidentsScreen extends ConsumerWidget {
  const SecurityIncidentsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(openIncidentsProvider);
    return AsyncResultView<List<SecurityIncidentSummary>>(
      asyncValue: async,
      builder: (context, incidents) =>
          _SecurityIncidentsBody(incidents: incidents),
    );
  }
}

class _SecurityIncidentsBody extends StatelessWidget {
  final List<SecurityIncidentSummary> incidents;
  const _SecurityIncidentsBody({required this.incidents});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(8),
          child: Align(
            alignment: Alignment.centerRight,
            child: OutlinedButton.icon(
              icon: const Icon(Icons.block_outlined),
              label: Text(l10n.securitySuppressAction),
              onPressed: () => showDialog<void>(
                context: context,
                builder: (_) => const _SuppressDialog(),
              ),
            ),
          ),
        ),
        Expanded(
          child: incidents.isEmpty
              ? Center(child: Text(l10n.genericEmpty))
              : ListView.builder(
                  itemCount: incidents.length,
                  itemBuilder: (context, index) =>
                      _IncidentRow(incident: incidents[index]),
                ),
        ),
      ],
    );
  }
}

class _IncidentRow extends ConsumerStatefulWidget {
  final SecurityIncidentSummary incident;
  const _IncidentRow({required this.incident});

  @override
  ConsumerState<_IncidentRow> createState() => _IncidentRowState();
}

class _IncidentRowState extends ConsumerState<_IncidentRow> {
  bool _closing = false;

  // No auth/session system exists yet (ADR 0021's own flagged
  // limitation) — same plain operator name every console mutation uses.
  static const _operator = 'console-operator';

  Future<void> _close() async {
    final l10n = AppLocalizations.of(context)!;
    setState(() => _closing = true);

    final client = ref.read(apiClientProvider);
    final result = await client.closeIncident(widget.incident.id, _operator);
    if (!mounted) return;

    switch (result) {
      case ApiOk():
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(l10n.securityIncidentClosed)));
      case ApiErr(:final failure):
        setState(() => _closing = false);
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(_failureMessage(failure, l10n))));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final incident = widget.incident;
    final dateFormat = DateFormat.yMd().add_Hm();
    final subtitle = [
      incident.status,
      dateFormat.format(incident.openedAt.toLocal()),
      l10n.securityColumnEvents(incident.eventCount),
    ].join('  •  ');

    return ListTile(
      leading: _SeverityIcon(severity: incident.severity),
      title: Text(incident.summary),
      subtitle: Text(subtitle),
      trailing: _closing
          ? const SizedBox(
              width: 20,
              height: 20,
              child: CircularProgressIndicator(strokeWidth: 2),
            )
          : IconButton(
              icon: const Icon(Icons.check_circle_outline),
              tooltip: l10n.securityIncidentCloseAction,
              onPressed: _close,
            ),
    );
  }
}

class _SeverityIcon extends StatelessWidget {
  final String severity;
  const _SeverityIcon({required this.severity});

  @override
  Widget build(BuildContext context) {
    // Renders whatever severity string the server sent, never
    // reclassifying it (FR-CONSOLE-001) — the icon/color choice below is
    // presentation, not a re-derivation of the verdict itself.
    final (icon, color) = switch (severity) {
      'CRITICAL' => (Icons.error, Colors.red),
      'HIGH' => (Icons.warning_amber, Colors.deepOrange),
      'MEDIUM' => (Icons.info_outline, Colors.orange),
      'LOW' => (Icons.info_outline, Colors.blueGrey),
      _ => (Icons.info_outline, Colors.grey),
    };
    return Icon(icon, color: color);
  }
}

class _SuppressDialog extends ConsumerStatefulWidget {
  const _SuppressDialog();

  @override
  ConsumerState<_SuppressDialog> createState() => _SuppressDialogState();
}

class _SuppressDialogState extends ConsumerState<_SuppressDialog> {
  final _formKey = GlobalKey<FormState>();
  final _detectorIdController = TextEditingController();
  final _resourceKindController = TextEditingController();
  final _resourceNameController = TextEditingController();
  final _reasonController = TextEditingController();
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _detectorIdController.dispose();
    _resourceKindController.dispose();
    _resourceNameController.dispose();
    _reasonController.dispose();
    super.dispose();
  }

  // No auth/session system exists yet (ADR 0021's own flagged
  // limitation) — same plain operator name every console mutation uses.
  static const _operator = 'console-operator';

  Future<void> _submit() async {
    final l10n = AppLocalizations.of(context)!;
    if (!(_formKey.currentState?.validate() ?? false)) return;

    final resourceKind = _resourceKindController.text.trim();
    final resourceName = _resourceNameController.text.trim();
    if (resourceKind.isEmpty != resourceName.isEmpty) {
      setState(() => _error = l10n.securitySuppressResourceBothOrNeither);
      return;
    }

    setState(() {
      _error = null;
      _submitting = true;
    });

    final client = ref.read(apiClientProvider);
    final result = await client.suppressDetector(
      detectorId: _detectorIdController.text.trim(),
      resourceKind: resourceKind.isEmpty ? null : resourceKind,
      resourceName: resourceName.isEmpty ? null : resourceName,
      reason: _reasonController.text.trim(),
      createdBy: _operator,
    );
    if (!mounted) return;

    switch (result) {
      case ApiOk():
        Navigator.of(context).pop();
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(l10n.securitySuppressSuccess)));
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
      title: Text(l10n.securitySuppressDialogTitle),
      content: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextFormField(
              controller: _detectorIdController,
              decoration: InputDecoration(
                labelText: l10n.securitySuppressDetectorId,
              ),
              validator: (v) => (v == null || v.trim().isEmpty) ? '' : null,
            ),
            TextFormField(
              controller: _resourceKindController,
              decoration: InputDecoration(
                labelText: l10n.securitySuppressResourceKind,
              ),
            ),
            TextFormField(
              controller: _resourceNameController,
              decoration: InputDecoration(
                labelText: l10n.securitySuppressResourceName,
              ),
            ),
            TextFormField(
              controller: _reasonController,
              decoration: InputDecoration(
                labelText: l10n.securitySuppressReason,
              ),
              validator: (v) => (v == null || v.trim().isEmpty) ? '' : null,
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
              : Text(l10n.securitySuppressSubmit),
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
