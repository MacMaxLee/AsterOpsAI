import 'package:flutter/material.dart';

/// A label/value line, reused across every data screen so field layout
/// stays consistent without each screen re-deciding it.
class MetricRow extends StatelessWidget {
  final String label;
  final Widget value;
  const MetricRow({required this.label, required this.value, super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            flex: 2,
            child: Text(label, style: Theme.of(context).textTheme.bodyMedium),
          ),
          Expanded(flex: 3, child: value),
        ],
      ),
    );
  }
}
