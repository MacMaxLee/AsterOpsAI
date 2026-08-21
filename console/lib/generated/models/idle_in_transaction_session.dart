// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class IdleInTransactionSession {
  final double idleDurationSeconds;
  final int pid;
  final String? username;

  const IdleInTransactionSession({
    required this.idleDurationSeconds,
    required this.pid,
    this.username,
  });

  static IdleInTransactionSession fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return IdleInTransactionSession(
      idleDurationSeconds: (map['idle_duration_seconds'] as num).toDouble(),
      pid: (map['pid'] as num).toInt(),
      username: map['username'] == null ? null : (map['username'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'idle_duration_seconds': idleDurationSeconds,
    'pid': pid,
    'username': username,
  };
}
