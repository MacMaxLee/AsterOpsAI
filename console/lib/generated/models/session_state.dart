// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

enum SessionState {
  active._('ACTIVE'),
  idle._('IDLE'),
  idleInTransaction._('IDLE_IN_TRANSACTION'),
  waiting._('WAITING');

  final String wireValue;
  const SessionState._(this.wireValue);

  static SessionState fromJson(dynamic json) {
    final value = json as String;
    return SessionState.values.firstWhere(
      (v) => v.wireValue == value,
      orElse: () => throw FormatException('Unknown SessionState: $value'),
    );
  }

  String toJson() => wireValue;
}
