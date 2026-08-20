// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'root_cause.dart';

final class RuledOut {
  final RootCause cause;
  final String reason;

  const RuledOut({required this.cause, required this.reason});

  static RuledOut fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return RuledOut(
      cause: RootCause.fromJson(map['cause']),
      reason: map['reason'] as String,
    );
  }

  Map<String, dynamic> toJson() => {'cause': cause.toJson(), 'reason': reason};
}
