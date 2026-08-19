import 'package:flutter/material.dart';

import '../generated/models/cpu_pressure.dart';
import '../generated/models/memory_pressure.dart';

/// Purely visual: maps the server's own NORMAL/ELEVATED/HIGH/CRITICAL tier
/// (already computed server-side, ADR 0006) to a color. No new thresholds
/// are computed here — this never re-derives the tier from a raw number.
class PressureBadge extends StatelessWidget {
  final String label;
  final String wireValue;
  const PressureBadge._({required this.label, required this.wireValue});

  factory PressureBadge.cpu(String label, CpuPressure pressure) =>
      PressureBadge._(label: label, wireValue: pressure.wireValue);

  factory PressureBadge.memory(String label, MemoryPressure pressure) =>
      PressureBadge._(label: label, wireValue: pressure.wireValue);

  Color _color(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return switch (wireValue) {
      'NORMAL' => Colors.green.shade600,
      'ELEVATED' => Colors.amber.shade700,
      'HIGH' => Colors.orange.shade800,
      'CRITICAL' => scheme.error,
      _ => scheme.onSurfaceVariant,
    };
  }

  @override
  Widget build(BuildContext context) {
    final color = _color(context);
    return Semantics(
      label: '$label: $wireValue',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 10,
            height: 10,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 6),
          Text('$label: $wireValue'),
        ],
      ),
    );
  }
}
