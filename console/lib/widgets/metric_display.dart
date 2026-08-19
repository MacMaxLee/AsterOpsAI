import 'package:flutter/material.dart';

import '../generated/models/capability.dart';
import '../generated/models/metric_value_for_double.dart';
import '../generated/models/metric_value_for_string.dart';
import '../generated/models/metric_value_for_uint64.dart';
import '../generated/models/self_metric_value_for_double.dart';
import '../generated/models/self_metric_value_for_uint64.dart';
import '../l10n/app_localizations.dart';

/// The one place `MetricValue`/`Capability`/`SelfMetricValue`'s wire states
/// (SUPPORTED/SAMPLE_GAP/COUNTER_RESET/UNAVAILABLE/LIMITED/
/// PERMISSION_REQUIRED) get turned into a UI treatment (U3 requirement 6 /
/// SRS FR-CAP-001) — every screen renders these through here rather than
/// re-deciding per-screen, and never falls back to a bare `0`/`"—"` for a
/// non-supported state. This is a straight vocabulary map off enums the
/// server already computed, not new client-side classification.
enum _Kind { supported, gap, reset, unavailable, limited, permissionRequired }

class _State {
  final _Kind kind;
  final String? valueText;
  final String? reason;
  const _State(this.kind, {this.valueText, this.reason});
}

_State _fromDouble(MetricValueForDouble v, String Function(double) format) =>
    switch (v) {
      MetricValueForDoubleSupported(:final value) => _State(
        _Kind.supported,
        valueText: format(value),
      ),
      MetricValueForDoubleSampleGap(:final reason) => _State(
        _Kind.gap,
        reason: reason,
      ),
      MetricValueForDoubleCounterReset(:final reason) => _State(
        _Kind.reset,
        reason: reason,
      ),
      MetricValueForDoubleUnavailable(:final reason) => _State(
        _Kind.unavailable,
        reason: reason,
      ),
    };

_State _fromUint64(MetricValueForUint64 v, String Function(int) format) =>
    switch (v) {
      MetricValueForUint64Supported(:final value) => _State(
        _Kind.supported,
        valueText: format(value),
      ),
      MetricValueForUint64SampleGap(:final reason) => _State(
        _Kind.gap,
        reason: reason,
      ),
      MetricValueForUint64CounterReset(:final reason) => _State(
        _Kind.reset,
        reason: reason,
      ),
      MetricValueForUint64Unavailable(:final reason) => _State(
        _Kind.unavailable,
        reason: reason,
      ),
    };

_State _fromStringMetric(MetricValueForString v) => switch (v) {
  MetricValueForStringSupported(:final value) => _State(
    _Kind.supported,
    valueText: value,
  ),
  MetricValueForStringSampleGap(:final reason) => _State(
    _Kind.gap,
    reason: reason,
  ),
  MetricValueForStringCounterReset(:final reason) => _State(
    _Kind.reset,
    reason: reason,
  ),
  MetricValueForStringUnavailable(:final reason) => _State(
    _Kind.unavailable,
    reason: reason,
  ),
};

_State _fromCapability(Capability v) => switch (v) {
  CapabilitySupported() => const _State(_Kind.supported),
  CapabilityLimited(:final reason) => _State(_Kind.limited, reason: reason),
  CapabilityUnavailable(:final reason) => _State(
    _Kind.unavailable,
    reason: reason,
  ),
  CapabilityPermissionRequired(:final reason) => _State(
    _Kind.permissionRequired,
    reason: reason,
  ),
};

_State _fromSelfDouble(
  SelfMetricValueForDouble v,
  String Function(double) format,
) => switch (v) {
  SelfMetricValueForDoubleSupported(:final value) => _State(
    _Kind.supported,
    valueText: format(value),
  ),
  SelfMetricValueForDoubleUnavailable(:final reason) => _State(
    _Kind.unavailable,
    reason: reason,
  ),
};

_State _fromSelfUint64(
  SelfMetricValueForUint64 v,
  String Function(int) format,
) => switch (v) {
  SelfMetricValueForUint64Supported(:final value) => _State(
    _Kind.supported,
    valueText: format(value),
  ),
  SelfMetricValueForUint64Unavailable(:final reason) => _State(
    _Kind.unavailable,
    reason: reason,
  ),
};

/// Renders one metric value inline (a labelled row is left to the caller —
/// this only renders the value cell itself).
class MetricValueText extends StatelessWidget {
  final _State _state;
  const MetricValueText._(this._state, {super.key});

  factory MetricValueText.double(
    MetricValueForDouble value,
    String Function(double) format, {
    Key? key,
  }) => MetricValueText._(_fromDouble(value, format), key: key);

  factory MetricValueText.uint64(
    MetricValueForUint64 value,
    String Function(int) format, {
    Key? key,
  }) => MetricValueText._(_fromUint64(value, format), key: key);

  factory MetricValueText.string(MetricValueForString value, {Key? key}) =>
      MetricValueText._(_fromStringMetric(value), key: key);

  factory MetricValueText.capability(Capability value, {Key? key}) =>
      MetricValueText._(_fromCapability(value), key: key);

  factory MetricValueText.selfDouble(
    SelfMetricValueForDouble value,
    String Function(double) format, {
    Key? key,
  }) => MetricValueText._(_fromSelfDouble(value, format), key: key);

  factory MetricValueText.selfUint64(
    SelfMetricValueForUint64 value,
    String Function(int) format, {
    Key? key,
  }) => MetricValueText._(_fromSelfUint64(value, format), key: key);

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final muted = theme.colorScheme.onSurfaceVariant;

    switch (_state.kind) {
      case _Kind.supported:
        return Text(_state.valueText ?? '');
      case _Kind.gap:
        return _badge(
          context,
          icon: Icons.hourglass_empty,
          label: l10n.metricStateSampleGapTitle,
          tooltip: _state.reason ?? l10n.metricStateSampleGapBody,
          color: muted,
        );
      case _Kind.reset:
        return _badge(
          context,
          icon: Icons.restart_alt,
          label: l10n.metricStateCounterResetTitle,
          tooltip: _state.reason ?? l10n.metricStateCounterResetBody,
          color: muted,
        );
      case _Kind.unavailable:
        return _badge(
          context,
          icon: Icons.remove_circle_outline,
          label: l10n.metricStateUnavailableTitle,
          tooltip: _state.reason ?? l10n.metricStateUnavailableTitle,
          color: muted,
        );
      case _Kind.limited:
        return _badge(
          context,
          icon: Icons.info_outline,
          label: l10n.metricStateLimitedTitle,
          tooltip: _state.reason ?? l10n.metricStateLimitedTitle,
          color: muted,
        );
      case _Kind.permissionRequired:
        return _badge(
          context,
          icon: Icons.lock_outline,
          label: l10n.metricStatePermissionRequiredTitle,
          tooltip: _state.reason ?? l10n.metricStatePermissionRequiredTitle,
          color: muted,
        );
    }
  }

  Widget _badge(
    BuildContext context, {
    required IconData icon,
    required String label,
    required String tooltip,
    required Color color,
  }) {
    return Tooltip(
      message: tooltip,
      child: Semantics(
        label: '$label: $tooltip',
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 16, color: color),
            const SizedBox(width: 4),
            Text(label, style: TextStyle(color: color)),
          ],
        ),
      ),
    );
  }
}
