// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'session_state.dart';

final class LongTransaction {
  final double durationSeconds;
  final int pid;
  final String? query;
  final SessionState state;
  final String? username;

  const LongTransaction({
    required this.durationSeconds,
    required this.pid,
    this.query,
    required this.state,
    this.username,
  });

  static LongTransaction fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return LongTransaction(
      durationSeconds: (map['duration_seconds'] as num).toDouble(),
      pid: (map['pid'] as num).toInt(),
      query: map['query'] == null ? null : (map['query'] as String),
      state: SessionState.fromJson(map['state']),
      username: map['username'] == null ? null : (map['username'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'duration_seconds': durationSeconds,
    'pid': pid,
    'query': query,
    'state': state.toJson(),
    'username': username,
  };
}
